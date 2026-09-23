//! Keeps Redrule current. It checks the public releases feed in the background, installs a new
//! version as soon as one is published, and restarts into it when asked. It never interrupts a
//! recording, and a version installed in the background starts with the next launch anyway.
use std::sync::Arc;
use std::time::Duration;

use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;

use crate::app::App;
use crate::shell::MAIN;

/// The menu item that checks for updates, then restarts into one that is installed.
pub const MENU_ID: &str = "check-updates";
/// Tells the main window that a new version is installed and waiting for a restart.
pub const READY_EVENT: &str = "update-ready";
const CHECK_TITLE: &str = "Check for Updates…";
const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

pub struct Updates {
    items: Vec<MenuItem<Wry>>,
    /// The version installed and waiting for a restart. Held while checking, so checks never overlap.
    installed: Mutex<Option<String>>,
}

impl Updates {
    pub fn new(items: Vec<MenuItem<Wry>>) -> Self {
        Self { items, installed: Mutex::new(None) }
    }
}

pub fn menu_item(handle: &AppHandle) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(handle, MENU_ID, CHECK_TITLE, true, None::<&str>)
}

/// Checks a minute after launch, then every six hours, without saying anything unless a version is installed.
pub fn check_in_background(handle: &AppHandle) {
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if let Err(error) = install_newer(&handle).await {
                log::warn!("Update check failed: {error}");
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// The menu item: restarts into an installed version, or checks now and reports what it found.
pub fn menu_clicked(handle: &AppHandle) {
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let already_installed = handle.state::<Updates>().installed.lock().await.is_some();
        if already_installed {
            restart(&handle);
            return;
        }
        let current = handle.package_info().version.to_string();
        match install_newer(&handle).await {
            Ok(Some(version)) => offer_restart(&handle, &version),
            Ok(None) => tell(&handle, "Redrule is up to date", &format!("Version {current} is the latest.")),
            Err(error) => tell(&handle, "Couldn't check for updates", &format!("{error}\n\nTry again later.")),
        }
    });
}

/// Restarts into the installed version, unless a meeting is being recorded.
pub fn restart(handle: &AppHandle) {
    if handle.state::<Arc<App>>().is_recording() {
        tell(
            handle,
            "A meeting is being recorded",
            "Redrule will use the new version the next time it starts. Stop the recording first to restart now.",
        );
        return;
    }
    handle.restart();
}

/// Downloads and installs a newer version if there is one. Returns the version waiting for a restart.
pub async fn install_newer(handle: &AppHandle) -> Result<Option<String>, String> {
    let updates = handle.state::<Updates>();
    let mut installed = updates.installed.lock().await;
    if installed.is_some() {
        return Ok(installed.clone());
    }
    let update = handle.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(update) = update else { return Ok(None) };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    let title = format!("Restart to Update to {}", update.version);
    for item in &updates.items {
        let _ = item.set_text(&title);
    }
    let _ = handle.emit_to(MAIN, READY_EVENT, &update.version);
    *installed = Some(update.version.clone());
    Ok(installed.clone())
}

fn offer_restart(handle: &AppHandle, version: &str) {
    let restart_handle = handle.clone();
    handle
        .dialog()
        .message(format!("Redrule {version} is installed. Restart now to start using it?"))
        .title("Update installed")
        .buttons(MessageDialogButtons::OkCancelCustom("Restart".into(), "Later".into()))
        .show(move |restart_now| {
            if restart_now {
                restart(&restart_handle);
            }
        });
}

fn tell(handle: &AppHandle, title: &str, message: &str) {
    handle.dialog().message(message).title(title).kind(MessageDialogKind::Info).show(|_| {});
}
