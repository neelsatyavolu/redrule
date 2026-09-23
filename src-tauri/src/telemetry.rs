//! Opt-in crash reports through Sentry. Nothing is sent unless the build has a DSN
//! (`REDRULE_SENTRY_DSN` at compile time) and the person turned on "Send crash reports".
//! Reports carry panics and uncaught webview errors with their stack traces, scrubbed of the user,
//! the host name, request data, home folder names and token-like strings. There are no
//! breadcrumbs, logs, sessions or performance traces.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use sentry::protocol::{DebugImage, Event, Exception, Frame, Level, Mechanism, Stacktrace};

/// Set by scripts/release.sh. Dev builds and forks have none and never report.
const DSN: Option<&str> = option_env!("REDRULE_SENTRY_DSN");
/// Webview errors reported per launch, so an error in a render loop can't flood the project.
const MAX_WEBVIEW_REPORTS: usize = 20;
const MAX_MESSAGE_CHARS: usize = 1_000;
const MAX_JS_FRAMES: usize = 50;
/// Shorter runs of letters and digits are words; longer ones with a digit are keys, tokens or ids.
const MIN_TOKEN_CHARS: usize = 32;
const REDACTED: &str = "[redacted]";

static ENABLED: AtomicBool = AtomicBool::new(false);
static STARTED: OnceLock<()> = OnceLock::new();
static WEBVIEW_REPORTS: AtomicUsize = AtomicUsize::new(0);

/// Reports go out only from builds with a DSN, and only while the setting is on.
fn should_report(dsn: Option<&str>, setting: bool) -> bool {
    setting && dsn.is_some_and(|dsn| !dsn.trim().is_empty())
}

/// Whether this build can send crash reports at all; the setting is hidden when it can't.
pub fn available() -> bool {
    should_report(DSN, true)
}

/// Applies the "Send crash reports" setting. The client starts the first time reports are turned
/// on, and turning them off stops sending right away.
pub fn configure(setting: bool) {
    let on = should_report(DSN, setting);
    if on {
        STARTED.get_or_init(start);
    }
    ENABLED.store(on, Ordering::SeqCst);
}

fn start() {
    let Some(dsn) = DSN.and_then(|dsn| dsn.trim().parse().ok()) else {
        return log::warn!("The crash report DSN is not valid, so crash reports are off.");
    };
    // `ClientOptions` is non-exhaustive, so it is filled in field by field.
    let mut options = sentry::ClientOptions::default();
    options.dsn = Some(dsn);
    options.release = Some(env!("CARGO_PKG_VERSION").into());
    options.environment = Some(if cfg!(debug_assertions) { "debug" } else { "release" }.into());
    options.send_default_pii = false;
    options.max_breadcrumbs = 0;
    options.auto_session_tracking = false;
    options.before_send = Some(Arc::new(|event| ENABLED.load(Ordering::SeqCst).then(|| scrub(event))));
    // Bound to the process hub, so every thread reports through it, whichever turned it on.
    let client = sentry::Client::from(sentry::apply_defaults(options));
    sentry::Hub::main().bind_client(Some(Arc::new(client)));
}

/// Reports an uncaught error from the webview, when reports are on.
pub fn report_webview_error(message: &str, stack: Option<&str>) {
    if !ENABLED.load(Ordering::SeqCst) || WEBVIEW_REPORTS.fetch_add(1, Ordering::SeqCst) >= MAX_WEBVIEW_REPORTS {
        return;
    }
    sentry::Hub::main().capture_event(webview_event(message, stack));
}

fn webview_event(message: &str, stack: Option<&str>) -> Event<'static> {
    let frames = stack.map(js_frames).unwrap_or_default();
    let exception = Exception {
        ty: "WebviewError".into(),
        value: Some(message.into()),
        stacktrace: (!frames.is_empty()).then(|| Stacktrace { frames, ..Default::default() }),
        mechanism: Some(Mechanism { ty: "onerror".into(), handled: Some(false), ..Default::default() }),
        ..Default::default()
    };
    Event { level: Level::Error, platform: "javascript".into(), exception: vec![exception].into(), ..Default::default() }
}

/// Frames from a WebKit stack, one "function@url:line:column" per line, innermost first.
fn js_frames(stack: &str) -> Vec<Frame> {
    let mut frames: Vec<Frame> = stack.lines().filter_map(js_frame).take(MAX_JS_FRAMES).collect();
    // Sentry lists the outermost call first.
    frames.reverse();
    frames
}

fn js_frame(line: &str) -> Option<Frame> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (function, location) = line.rsplit_once('@').unwrap_or(("", line));
    let mut parts = location.rsplitn(3, ':');
    let (column, row, file) = (parts.next(), parts.next(), parts.next());
    let (file, lineno, colno) = match (file, row.and_then(|r| r.parse().ok()), column.and_then(|c| c.parse().ok())) {
        (Some(file), Some(lineno), Some(colno)) => (file, Some(lineno), Some(colno)),
        _ => (location, None, None),
    };
    Some(Frame {
        function: (!function.is_empty()).then(|| function.to_string()),
        abs_path: Some(file.to_string()),
        lineno,
        colno,
        ..Default::default()
    })
}

/// Keeps what a crash needs (messages, stack traces, OS and device model, app version) and drops
/// everything that could identify the person or carry meeting content.
fn scrub(mut event: Event<'static>) -> Event<'static> {
    event.user = None;
    event.request = None;
    event.server_name = None;
    event.transaction = None;
    event.template = None;
    event.breadcrumbs = Default::default();
    event.extra.clear();
    event.tags.clear();
    event.modules.clear();
    event.contexts.retain(|name, _| matches!(name.as_str(), "os" | "device" | "rust"));
    event.message = event.message.as_deref().map(scrub_text);
    event.culprit = event.culprit.as_deref().map(scrub_text);
    if let Some(entry) = event.logentry.as_mut() {
        entry.message = scrub_text(&entry.message);
        entry.params.clear();
    }
    for exception in &mut event.exception.values {
        exception.value = exception.value.as_deref().map(scrub_text);
        exception.stacktrace.iter_mut().chain(exception.raw_stacktrace.iter_mut()).for_each(scrub_stacktrace);
    }
    for thread in &mut event.threads.values {
        thread.stacktrace.iter_mut().chain(thread.raw_stacktrace.iter_mut()).for_each(scrub_stacktrace);
    }
    event.stacktrace.iter_mut().for_each(scrub_stacktrace);
    for image in &mut event.debug_meta.to_mut().images {
        match image {
            DebugImage::Apple(image) => image.name = home_to_tilde(&image.name),
            DebugImage::Symbolic(image) => {
                image.name = home_to_tilde(&image.name);
                image.debug_file = image.debug_file.as_deref().map(home_to_tilde);
            }
            _ => {}
        }
    }
    event
}

fn scrub_stacktrace(stacktrace: &mut Stacktrace) {
    for frame in &mut stacktrace.frames {
        frame.abs_path = frame.abs_path.as_deref().map(home_to_tilde);
        frame.filename = frame.filename.as_deref().map(home_to_tilde);
        frame.package = frame.package.as_deref().map(home_to_tilde);
        frame.vars.clear();
        frame.pre_context.clear();
        frame.context_line = None;
        frame.post_context.clear();
    }
}

/// A message with home folders shortened, token-like strings removed, and its length capped.
fn scrub_text(text: &str) -> String {
    redact_tokens(&home_to_tilde(text)).chars().take(MAX_MESSAGE_CHARS).collect()
}

/// "/Users/<name>/…" becomes "~/…", so reports never carry the account name.
fn home_to_tilde(text: &str) -> String {
    const HOME: &str = "/Users/";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(HOME) {
        out.push_str(&rest[..at]);
        out.push('~');
        let after = &rest[at + HOME.len()..];
        let name_end = after.find(|c: char| !(c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))).unwrap_or(after.len());
        rest = &after[name_end..];
    }
    out.push_str(rest);
    out
}

/// Replaces long runs of letters and digits that contain a digit, such as API keys, bearer tokens
/// and ids.
fn redact_tokens(text: &str) -> String {
    let is_token_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '=');
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(is_token_char) {
        out.push_str(&rest[..start]);
        let run = &rest[start..];
        let end = run.find(|c: char| !is_token_char(c)).unwrap_or(run.len());
        let word = &run[..end];
        let secret = word.len() >= MIN_TOKEN_CHARS && word.chars().any(|c| c.is_ascii_digit());
        out.push_str(if secret { REDACTED } else { word });
        rest = &run[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::{Breadcrumb, Context, Request, User};

    const TEST_DSN: &str = "https://public@o0.ingest.sentry.io/0";

    #[test]
    fn reports_only_with_a_compiled_dsn_and_the_setting_on() {
        assert!(should_report(Some(TEST_DSN), true));
        assert!(!should_report(Some(TEST_DSN), false));
        assert!(!should_report(None, true));
        assert!(!should_report(Some("  "), true));
        assert!(!should_report(None, false));
    }

    #[test]
    fn home_folders_become_a_tilde() {
        assert_eq!(home_to_tilde("/Users/jane/Library/Redrule/a.md"), "~/Library/Redrule/a.md");
        assert_eq!(home_to_tilde("open /Users/jane.doe and file:///Users/bob/x"), "open ~ and file://~/x");
        assert_eq!(home_to_tilde("/usr/lib/libSystem.dylib"), "/usr/lib/libSystem.dylib");
    }

    #[test]
    fn keys_and_tokens_are_redacted_but_words_stay() {
        let key = "sk-proj-4f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c"; // gitleaks:allow (fake key)
        assert_eq!(redact_tokens(&format!("Bearer {key} failed")), "Bearer [redacted] failed");
        assert_eq!(redact_tokens("index out of bounds: the len is 3 but the index is 7"), "index out of bounds: the len is 3 but the index is 7");
        assert_eq!(redact_tokens("minutes_lib::app::recording::finish_recording_and_write"), "minutes_lib::app::recording::finish_recording_and_write");
    }

    #[test]
    fn scrub_drops_personal_fields_and_shortens_paths() {
        let frame = Frame {
            abs_path: Some("/Users/jane/src/lib.rs".into()),
            package: Some("/Users/jane/Applications/Redrule.app/Contents/MacOS/minutes".into()),
            ..Default::default()
        };
        let mut event = Event {
            message: Some("could not read /Users/jane/Documents/Weekly sync.md".into()),
            server_name: Some("Neels-MacBook-Pro".into()),
            user: Some(User { username: Some("neel".into()), ..Default::default() }),
            request: Some(Request { url: "https://example.com".parse().ok(), ..Default::default() }),
            breadcrumbs: vec![Breadcrumb { message: Some("Weekly sync".into()), ..Default::default() }].into(),
            exception: vec![Exception {
                value: Some("token 0123456789abcdef0123456789abcdef at /Users/jane/x".into()),
                stacktrace: Some(Stacktrace { frames: vec![frame], ..Default::default() }),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };
        event.extra.insert("transcript".into(), "hello".into());
        event.tags.insert("meeting".into(), "Weekly sync".into());
        event.contexts.insert("os".into(), Context::Os(Box::default()));
        event.contexts.insert("meeting".into(), Context::Other(Default::default()));

        let event = scrub(event);
        assert_eq!(event.message.as_deref(), Some("could not read ~/Documents/Weekly sync.md"));
        assert!(event.server_name.is_none() && event.user.is_none() && event.request.is_none());
        assert!(event.breadcrumbs.values.is_empty() && event.extra.is_empty() && event.tags.is_empty());
        assert_eq!(event.contexts.keys().collect::<Vec<_>>(), ["os"]);
        let exception = &event.exception.values[0];
        assert_eq!(exception.value.as_deref(), Some("token [redacted] at ~/x"));
        let frame = &exception.stacktrace.as_ref().unwrap().frames[0];
        assert_eq!(frame.abs_path.as_deref(), Some("~/src/lib.rs"));
        assert_eq!(frame.package.as_deref(), Some("~/Applications/Redrule.app/Contents/MacOS/minutes"));
    }

    #[test]
    fn webkit_stacks_become_frames_outermost_first() {
        let stack = "render@tauri://localhost/assets/index-a1.js:12:345\nglobal code@tauri://localhost/assets/index-a1.js:1:2\n[native code]";
        let frames = js_frames(stack);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].abs_path.as_deref(), Some("[native code]"));
        assert_eq!(frames[2].function.as_deref(), Some("render"));
        assert_eq!(frames[2].abs_path.as_deref(), Some("tauri://localhost/assets/index-a1.js"));
        assert_eq!((frames[2].lineno, frames[2].colno), (Some(12), Some(345)));
    }
}
