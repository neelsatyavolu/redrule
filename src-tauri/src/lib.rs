pub use minutes_core as core;

mod app;
mod commands;
pub mod platform;
pub mod providers;
mod shell;
mod telemetry;
mod updates;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};

use app::App;

pub fn run() {
    // First, so panics while starting up are reported too (only when the person opted in).
    telemetry::configure(app::crash_reports_on());
    let app = tauri::Builder::default()
        // A second launch focuses the running copy instead of starting another recorder.
        .plugin(tauri_plugin_single_instance::init(|handle, _, _| shell::show_main(handle)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|tauri_app| {
            let handle = tauri_app.handle().clone();
            let support = core::store::support_folder().unwrap_or_else(|_| dirs::data_dir().unwrap_or_default().join("Redrule"));
            let models = support.join("models");
            let app = Arc::new(App::new(handle.clone(), models));
            tauri_app.manage(Arc::clone(&app));
            tauri_app.manage(shell::install(&handle)?);
            app.start();
            updates::check_in_background(&handle);
            // The page shows the window once it has painted; if it never does, show it anyway.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                let hidden = handle.get_webview_window(shell::MAIN).is_some_and(|w| !w.is_visible().unwrap_or(true));
                if hidden {
                    shell::show_main(&handle);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != shell::MAIN {
                return;
            }
            match event {
                // Redrule keeps watching for calls from the menu bar after its window is closed.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    shell::hide_to_menu_bar(window.app_handle());
                }
                WindowEvent::Focused(true) => window.state::<Arc<App>>().refresh_permissions(),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_ready,
            commands::get_state,
            commands::meeting_detail,
            commands::start_recording,
            commands::stop_recording,
            commands::generate_notes,
            commands::ask_meeting,
            commands::search_meetings,
            commands::copy_consent_notice,
            commands::rename_meeting,
            commands::set_archived,
            commands::set_tags,
            commands::set_meeting_folder,
            commands::create_folder,
            commands::join_folder,
            commands::rename_folder,
            commands::reset_folder_link,
            commands::delete_folder,
            commands::leave_folder,
            commands::copy_folder_link,
            commands::delete_meeting,
            commands::save_note,
            commands::copy_markdown,
            commands::export_meeting,
            commands::rename_speaker,
            commands::publish_share,
            commands::revoke_share,
            commands::connect,
            commands::submit_pasted_code,
            commands::cancel_connecting,
            commands::disconnect,
            commands::save_api_key,
            commands::remove_api_key,
            commands::request_microphone,
            commands::request_screen_recording,
            commands::request_calendar,
            commands::update_settings,
            commands::microphones,
            commands::model_choices,
            commands::retry_speech_model,
            commands::retry_note_model,
            commands::local_models,
            commands::remove_local_model,
            commands::dismiss_banner,
            commands::show_main_window,
            commands::reveal_meeting,
            commands::restart_to_update,
            commands::report_error,
        ])
        .build(tauri::generate_context!())
        .expect("Redrule could not start");

    app.run(|handle, event| match event {
        // Clicking the Dock icon brings the window back.
        RunEvent::Reopen { .. } => shell::show_main(handle),
        // Quitting from the Dock or the system asks first while a meeting is being recorded.
        RunEvent::ExitRequested { code: None, api, .. } if handle.state::<Arc<App>>().is_recording() => {
            api.prevent_exit();
            shell::confirm_quit(handle);
        }
        _ => {}
    });
}
