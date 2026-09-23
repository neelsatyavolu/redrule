//! The webview's entry points. Each command is a thin call into `App`.
use std::sync::Arc;

use minutes_engine::Microphone;
use serde::Serialize;
use tauri::State;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::app::{App, LocalModels, MeetingDetail, ModelKind, Settings, SettingsPatch, State as Snapshot};
use crate::core::Result;
use crate::core::api_providers::ApiProvider;
use crate::core::ask::Exchange;
use crate::core::export::ExportFormat;
use crate::core::models::{MeetingApp, MeetingNote};
use crate::core::oauth::ProviderId;
use crate::core::search::SearchHit;
use crate::platform::{calendar, permissions};
use crate::providers::clients::{model_choices as available_models, refresh_model_choices, ModelChoice};
use crate::shell;
use crate::telemetry;
use crate::updates;

type AppState<'a> = State<'a, Arc<App>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    id: String,
    #[serde(flatten)]
    choice: ModelChoice,
}

/// Shows the main window once the page has painted, so it never flashes blank.
#[tauri::command]
pub fn app_ready(handle: tauri::AppHandle) {
    shell::show_main(&handle);
}

#[tauri::command]
pub fn get_state(app: AppState) -> Snapshot {
    app.refresh_permissions();
    app.snapshot()
}

#[tauri::command]
pub fn meeting_detail(app: AppState, id: String) -> Result<MeetingDetail> {
    app.meeting_detail(&id)
}

#[tauri::command]
pub async fn start_recording(app: AppState<'_>, source: Option<MeetingApp>, folder: Option<String>) -> Result<()> {
    app.start_recording(source.unwrap_or(MeetingApp::Manual), folder).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(app: AppState<'_>) -> Result<()> {
    let app = Arc::clone(&app);
    // Finishing the transcript and notes takes a while; the UI follows along through state events.
    tauri::async_runtime::spawn(async move { app.stop_recording().await });
    Ok(())
}

#[tauri::command]
pub async fn generate_notes(app: AppState<'_>, id: String) -> Result<()> {
    let meeting = app.editable(&id)?;
    let app = Arc::clone(&app);
    tauri::async_runtime::spawn(async move { app.generate_notes(&meeting).await });
    Ok(())
}

#[tauri::command]
pub async fn ask_meeting(app: AppState<'_>, id: String, question: String, history: Vec<Exchange>) -> Result<String> {
    app.ask_meeting(&id, &question, &history).await
}

/// Blocking: the first search reads every meeting's notes and transcript from disk.
#[tauri::command]
pub async fn search_meetings(app: AppState<'_>, query: String) -> Result<Vec<SearchHit>> {
    let app = Arc::clone(app.inner());
    tauri::async_runtime::spawn_blocking(move || app.search_meetings(&query))
        .await
        .map_err(|error| crate::core::Error::message(error.to_string()))
}

#[tauri::command]
pub fn copy_consent_notice(handle: tauri::AppHandle, app: AppState) -> Result<()> {
    let notice = app.read(|state| state.settings.consent_notice.clone());
    handle.clipboard().write_text(notice).map_err(|e| crate::core::Error::message(format!("The notice could not be copied. {e}")))
}

#[tauri::command]
pub fn rename_meeting(app: AppState, id: String, title: String) -> Result<()> {
    app.rename_meeting(&id, &title)
}

#[tauri::command]
pub fn set_archived(app: AppState, id: String, archived: bool) -> Result<()> {
    app.set_archived(&id, archived)
}

#[tauri::command]
pub fn set_tags(app: AppState, id: String, tags: Vec<String>) -> Result<()> {
    app.set_tags(&id, tags)
}

#[tauri::command]
pub fn set_meeting_folder(app: AppState, id: String, folder_id: Option<String>) -> Result<()> {
    app.set_meeting_folder(&id, folder_id)
}

#[tauri::command]
pub async fn create_folder(app: AppState<'_>, name: String, your_name: String) -> Result<String> {
    app.create_folder(&name, &your_name).await
}

#[tauri::command]
pub async fn join_folder(app: AppState<'_>, link: String, your_name: String) -> Result<String> {
    app.join_folder(&link, &your_name).await
}

#[tauri::command]
pub async fn rename_folder(app: AppState<'_>, id: String, name: String) -> Result<()> {
    app.rename_folder(&id, &name).await
}

#[tauri::command]
pub async fn reset_folder_link(app: AppState<'_>, id: String) -> Result<String> {
    app.reset_folder_link(&id).await
}

#[tauri::command]
pub async fn delete_folder(app: AppState<'_>, id: String) -> Result<()> {
    app.delete_folder(&id).await
}

#[tauri::command]
pub async fn leave_folder(app: AppState<'_>, id: String) -> Result<()> {
    app.leave_folder(&id).await
}

#[tauri::command]
pub fn copy_folder_link(app: AppState, id: String) -> Result<()> {
    app.copy_folder_link(&id)
}

#[tauri::command]
pub async fn delete_meeting(app: AppState<'_>, id: String) -> Result<()> {
    app.delete_meeting(&id).await
}

#[tauri::command]
pub fn save_note(app: AppState, id: String, note: MeetingNote) -> Result<()> {
    app.save_note(&id, note)
}

#[tauri::command]
pub fn copy_markdown(app: AppState, id: String) -> Result<()> {
    app.copy_markdown(&id)
}

#[tauri::command]
pub async fn export_meeting(app: AppState<'_>, id: String, format: ExportFormat) -> Result<bool> {
    let app = Arc::clone(app.inner());
    tauri::async_runtime::spawn_blocking(move || app.export_meeting(&id, format))
        .await
        .map_err(|error| crate::core::Error::message(error.to_string()))?
}

#[tauri::command]
pub fn rename_speaker(app: AppState, id: String, key: String, name: String) -> Result<()> {
    app.rename_speaker(&id, &key, &name)
}

#[tauri::command]
pub async fn publish_share(app: AppState<'_>, id: String, include_transcript: bool) -> Result<String> {
    app.publish_share(&id, include_transcript).await
}

#[tauri::command]
pub async fn revoke_share(app: AppState<'_>, id: String) -> Result<()> {
    app.revoke_share(&id).await
}

#[tauri::command]
pub fn connect(app: AppState, provider: ProviderId) -> Result<()> {
    app.connect(provider)
}

#[tauri::command]
pub fn submit_pasted_code(app: AppState, text: String) {
    app.submit_pasted_code(text);
}

#[tauri::command]
pub fn cancel_connecting(app: AppState) {
    app.cancel_connecting();
}

#[tauri::command]
pub fn disconnect(app: AppState, provider: ProviderId) {
    app.disconnect(provider);
}

/// Checks the key with the provider before saving it. `base_url` and `model` are for a custom server.
#[tauri::command]
pub async fn save_api_key(app: AppState<'_>, provider: ApiProvider, key: String, base_url: String, model: String) -> Result<()> {
    app.save_api_key(provider, &key, &base_url, &model).await
}

#[tauri::command]
pub fn remove_api_key(app: AppState, provider: ApiProvider) -> Result<()> {
    app.remove_api_key(provider)
}

#[tauri::command]
pub async fn request_microphone(app: AppState<'_>) -> Result<()> {
    permissions::request_microphone().await;
    app.refresh_permissions();
    Ok(())
}

#[tauri::command]
pub fn request_screen_recording(app: AppState) {
    permissions::request_screen_recording();
    app.refresh_permissions();
}

/// Optional: lets recordings take their calendar event's title and attendees.
#[tauri::command]
pub async fn request_calendar(handle: tauri::AppHandle, app: AppState<'_>) -> Result<()> {
    // Waits for the person to answer the system prompt.
    let prompted = tauri::async_runtime::spawn_blocking(calendar::request).await.unwrap_or(false);
    app.refresh_permissions();
    // Answering the prompt hands focus to the app before it, which leaves the window behind
    // other windows when Redrule has no Dock icon to click.
    if prompted {
        shell::show_main(&handle);
    }
    Ok(())
}

/// The calendars to choose from in settings; empty until access is allowed.
#[tauri::command]
pub async fn calendars() -> Vec<calendar::Calendar> {
    tauri::async_runtime::spawn_blocking(calendar::calendars).await.unwrap_or_default()
}

#[tauri::command]
pub fn update_settings(handle: tauri::AppHandle, app: AppState, patch: SettingsPatch) -> Settings {
    let dock_changed = patch.show_in_dock.is_some();
    app.apply_settings(patch);
    if dock_changed {
        crate::shell::sync_dock_visibility(&handle);
    }
    app.use_chosen_models();
    let settings = app.read(|state| state.settings.clone());
    telemetry::configure(settings.crash_reports);
    settings
}

#[tauri::command]
pub async fn local_models(app: AppState<'_>) -> Result<LocalModels> {
    let app = Arc::clone(app.inner());
    tauri::async_runtime::spawn_blocking(move || app.local_models())
        .await
        .map_err(|error| crate::core::Error::message(error.to_string()))
}

#[tauri::command]
pub fn remove_local_model(app: AppState, kind: ModelKind, id: String) -> Result<()> {
    app.remove_local_model(kind, &id)
}

#[tauri::command]
pub async fn microphones() -> Vec<Microphone> {
    tauri::async_runtime::spawn_blocking(minutes_engine::microphones).await.unwrap_or_default()
}

#[tauri::command]
pub async fn model_choices() -> Vec<ModelOption> {
    refresh_model_choices().await;
    available_models().into_iter().map(|choice| ModelOption { id: choice.id(), choice }).collect()
}

#[tauri::command]
pub fn retry_speech_model(app: AppState) {
    app.prepare_speech_model();
}

#[tauri::command]
pub fn retry_note_model(app: AppState) {
    app.prepare_note_model();
}

#[tauri::command]
pub fn dismiss_banner(app: AppState) {
    app.set_banner(None);
}

/// Checks for a newer version now and installs it. Returns the version waiting for a restart, if any.
#[tauri::command]
pub async fn check_for_updates(handle: tauri::AppHandle) -> Result<Option<String>> {
    updates::install_newer(&handle).await.map_err(crate::core::Error::message)
}

/// Restarts into the version the updater installed in the background.
#[tauri::command]
pub fn restart_to_update(handle: tauri::AppHandle) {
    updates::restart(&handle);
}

#[tauri::command]
pub fn show_main_window(handle: tauri::AppHandle) {
    shell::show_main(&handle);
}

#[tauri::command]
pub fn reveal_meeting(app: AppState, id: String) -> Result<()> {
    let folder = app.store()?.folder(&id)?;
    std::process::Command::new("/usr/bin/open").arg(folder).spawn()?;
    Ok(())
}

/// An uncaught error in the webview, sent only when crash reports are on.
#[tauri::command]
pub fn report_error(message: String, stack: Option<String>) {
    telemetry::report_webview_error(&message, stack.as_deref());
}
