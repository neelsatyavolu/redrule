//! Native chrome: the menu bar, the tray item, the main window and the floating call banner.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{ActivationPolicy, AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, Wry};
use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask, tauri_panel};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::app::App;
use crate::core::models::MeetingApp;

pub const MAIN: &str = "main";
const BANNER: &str = "banner";
const BANNER_SIZE: (f64, f64) = (372.0, 132.0);
const BANNER_MARGIN: f64 = 14.0;
/// Asks the main window to open its settings sheet.
pub const OPEN_SETTINGS_EVENT: &str = "open-settings";

tauri_panel! {
    panel!(BannerPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

/// Menu items whose title follows the recording state.
pub struct RecordItems {
    menu: MenuItem<Wry>,
    tray: MenuItem<Wry>,
    tray_icon: TrayIcon<Wry>,
    showing_recording: AtomicBool,
}

pub fn install(app: &AppHandle) -> tauri::Result<RecordItems> {
    let menu_record = MenuItem::with_id(app, "record", "Record Meeting", true, Some("CmdOrCtrl+Shift+R"))?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit Redrule", true, Some("CmdOrCtrl+Q"))?;
    let app_menu = Submenu::with_items(
        app,
        "Redrule",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Redrule"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let file = Submenu::with_items(app, "File", true, &[&menu_record, &PredefinedMenuItem::close_window(app, None)?])?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[&PredefinedMenuItem::minimize(app, None)?, &PredefinedMenuItem::maximize(app, None)?],
    )?;
    app.set_menu(Menu::with_items(app, &[&app_menu, &file, &edit, &window])?)?;

    let tray_record = MenuItem::with_id(app, "record", "Record meeting", true, None::<&str>)?;
    let tray_menu = Menu::with_items(
        app,
        &[
            &tray_record,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "open", "Open Redrule", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit Redrule", true, None::<&str>)?,
        ],
    )?;
    let tray_icon = TrayIconBuilder::with_id("minutes")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Redrule")
        .menu(&tray_menu)
        .show_menu_on_left_click(true)
        .build(app)?;

    app.on_menu_event(|app, event| handle_menu(app, event.id.as_ref()));
    create_banner(app)?;
    Ok(RecordItems { menu: menu_record, tray: tray_record, tray_icon, showing_recording: AtomicBool::new(false) })
}

impl RecordItems {
    pub fn sync(&self, recording: bool) {
        if self.showing_recording.swap(recording, Ordering::Relaxed) == recording {
            return;
        }
        let (menu, tray) = if recording { ("Stop Recording", "Stop and write notes") } else { ("Record Meeting", "Record meeting") };
        let _ = self.menu.set_text(menu);
        let _ = self.tray.set_text(tray);
        let icon: &[u8] =
            if recording { include_bytes!("../icons/tray-recording.png") } else { include_bytes!("../icons/tray.png") };
        if let Ok(image) = Image::from_bytes(icon) {
            let _ = self.tray_icon.set_icon(Some(image));
            let _ = self.tray_icon.set_icon_as_template(!recording);
        }
    }
}

fn handle_menu(handle: &AppHandle, id: &str) {
    let app = Arc::clone(&*handle.state::<Arc<App>>());
    match id {
        "record" => {
            tauri::async_runtime::spawn(async move {
                if app.is_recording() {
                    app.stop_recording().await;
                } else {
                    app.start_recording(MeetingApp::Manual).await;
                }
            });
        }
        "open" => show_main(handle),
        "settings" => {
            show_main(handle);
            let _ = handle.emit_to(MAIN, OPEN_SETTINGS_EVENT, ());
        }
        "quit" => confirm_quit(handle),
        _ => {}
    }
}

/// Brings the window back, and the Dock icon with it.
pub fn show_main(handle: &AppHandle) {
    let _ = handle.set_activation_policy(ActivationPolicy::Regular);
    if let Some(window) = handle.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Closing the window leaves Redrule running in the menu bar only: it keeps watching for calls,
/// the call banner still offers to record, and the tray item starts and stops recordings.
pub fn hide_to_menu_bar(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(MAIN) {
        let _ = window.hide();
    }
    let _ = handle.set_activation_policy(ActivationPolicy::Accessory);
}

/// Quitting mid-recording loses the rest of the meeting, so it asks first.
pub fn confirm_quit(handle: &AppHandle) {
    let app = Arc::clone(&*handle.state::<Arc<App>>());
    if !app.is_recording() {
        handle.exit(0);
        return;
    }
    let handle = handle.clone();
    handle
        .dialog()
        .message("Quitting now stops the recording without writing notes. The transcript so far is kept.")
        .title("A meeting is being recorded")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Keep Recording".into(), "Quit".into()))
        .show({
            let handle = handle.clone();
            move |keep_recording| {
                if !keep_recording {
                    handle.exit(0);
                }
            }
        });
}

/// The call prompt: a borderless panel in the top-right corner that never takes focus away from the call.
fn create_banner(app: &AppHandle) -> tauri::Result<()> {
    let (width, height) = BANNER_SIZE;
    let position = app
        .primary_monitor()?
        .map(|monitor| {
            let scale = monitor.scale_factor();
            let area = monitor.work_area();
            let right = (area.position.x as f64 + area.size.width as f64) / scale;
            LogicalPosition::new(right - width - BANNER_MARGIN, area.position.y as f64 / scale + BANNER_MARGIN)
        })
        .unwrap_or(LogicalPosition::new(BANNER_MARGIN, BANNER_MARGIN));
    PanelBuilder::<_, BannerPanel>::new(app, BANNER)
        .url(WebviewUrl::App("banner.html".into()))
        .title("Redrule")
        .size(tauri::Size::Logical(LogicalSize::new(width, height)))
        .position(tauri::Position::Logical(position))
        .level(PanelLevel::Status)
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .collection_behavior(CollectionBehavior::new().can_join_all_spaces().full_screen_auxiliary().stationary())
        .hides_on_deactivate(false)
        .has_shadow(true)
        .transparent(true)
        .corner_radius(16.0)
        .with_window(|window| window.accept_first_mouse(true).decorations(false).visible(false).resizable(false))
        .build()
        .map(|_| ())
}

pub fn show_banner(handle: &AppHandle, visible: bool) {
    let handle_for_main = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        let Ok(panel) = handle_for_main.get_webview_panel(BANNER) else { return };
        if visible {
            panel.show();
        } else {
            panel.hide();
        }
    });
}
