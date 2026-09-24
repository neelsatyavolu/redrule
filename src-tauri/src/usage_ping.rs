//! A daily anonymous usage ping to analytics.n3el.dev, on unless the person turns off
//! "Share anonymous usage stats". It carries a random install id, the app version, the macOS
//! version and the chip type. Nothing about meetings, accounts or the person is sent.
use std::time::Duration;

use objc2_foundation::NSProcessInfo;
use serde::Serialize;

use crate::app::{set_usage_last_sent_day, usage_install_id, usage_last_sent_day, usage_stats_on};

const ENDPOINT: &str = "https://analytics.n3el.dev/v1/heartbeat";
const PRODUCT: &str = "redrule";
/// Long enough to stay out of the way of startup.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// Checks hourly, so a Mac left running still pings once a day; it sends at most once per UTC day.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, PartialEq)]
struct Heartbeat<'a> {
    product: &'a str,
    install_id: &'a str,
    version: &'a str,
    platform: &'a str,
    os_version: &'a str,
    arch: &'a str,
    channel: &'a str,
}

/// Sends when the setting is on and nothing was sent yet on this UTC day.
fn due(enabled: bool, last_sent_day: Option<&str>, today: &str) -> bool {
    enabled && last_sent_day != Some(today)
}

/// The analytics service names Apple silicon "arm64".
fn arch_name(arch: &str) -> &str {
    if arch == "aarch64" { "arm64" } else { arch }
}

fn os_version() -> String {
    let version = NSProcessInfo::processInfo().operatingSystemVersion();
    format!("{}.{}", version.majorVersion, version.minorVersion)
}

pub fn start() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(FIRST_CHECK).await;
        let http = reqwest::Client::new();
        loop {
            send_if_due(&http).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Fire and forget: a failed ping is tried again at the next hourly check, never sooner.
async fn send_if_due(http: &reqwest::Client) {
    let today = chrono::Utc::now().date_naive().to_string();
    if !due(usage_stats_on(), usage_last_sent_day().as_deref(), &today) {
        return;
    }
    let install_id = usage_install_id();
    let os_version = os_version();
    let body = Heartbeat {
        product: PRODUCT,
        install_id: &install_id,
        version: env!("CARGO_PKG_VERSION"),
        platform: "macos",
        os_version: &os_version,
        arch: arch_name(std::env::consts::ARCH),
        channel: if cfg!(debug_assertions) { "dev" } else { "release" },
    };
    let sent = http.post(ENDPOINT).json(&body).timeout(TIMEOUT).send().await;
    if sent.is_ok_and(|response| response.status().is_success()) {
        set_usage_last_sent_day(&today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_once_per_utc_day_and_never_when_turned_off() {
        assert!(due(true, None, "2026-09-23"));
        assert!(due(true, Some("2026-09-22"), "2026-09-23"));
        assert!(!due(true, Some("2026-09-23"), "2026-09-23"));
        assert!(!due(false, None, "2026-09-23"));
        assert!(!due(false, Some("2026-09-22"), "2026-09-23"));
    }

    #[test]
    fn the_body_has_only_the_heartbeat_fields() {
        let body = Heartbeat {
            product: PRODUCT,
            install_id: "00000000-0000-4000-8000-000000000000",
            version: "0.3.5",
            platform: "macos",
            os_version: "26.0",
            arch: arch_name("aarch64"),
            channel: "release",
        };
        let json = serde_json::to_value(&body).unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["arch", "channel", "install_id", "os_version", "platform", "product", "version"]);
        assert_eq!(json["arch"], "arm64");
        assert_eq!(arch_name("x86_64"), "x86_64");
    }
}
