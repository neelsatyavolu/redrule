//! The webview's entry points. Each command is a thin call into `App`.
use std::sync::Arc;

use minutes_engine::Microphone;
use serde::Serialize;
use tauri::State;

use crate::app::{App, LocalModels, MeetingDetail, ModelKind, Settings, SettingsPatch, State as Snapshot};
use crate::core::Result;
use crate::core::models::{MeetingApp, MeetingNote};
use crate::core::oauth::ProviderId;
use crate::platform::permissions;
use crate::providers::clients::{model_choices as available_models, refresh_model_choices, ModelChoice};
use crate::shell;
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
pub async fn start_recording(app: AppState<'_>, source: Option<MeetingApp>) -> Result<()> {
    app.start_recording(source.unwrap_or(MeetingApp::Manual)).await;
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
pub fn rename_meeting(app: AppState, id: String, title: String) -> Result<()> {
    app.rename_meeting(&id, &title)
}

#[tauri::command]
pub fn set_archived(app: AppState, id: String, archived: bool) -> Result<()> {
    app.set_archived(&id, archived)
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

#[tauri::command]
pub fn update_settings(app: AppState, patch: SettingsPatch) -> Settings {
    app.apply_settings(patch);
    app.use_chosen_models();
    app.read(|state| state.settings.clone())
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
pub fn dismiss_banner(app: AppState) {
    app.set_banner(None);
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
