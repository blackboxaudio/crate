use serde::Deserialize;
use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, MenuItemKind, Submenu, SubmenuBuilder},
    AppHandle, Emitter, Manager, Wry,
};

/// Get the application name based on the environment
/// - production: "Crate"
/// - development: "Crate Development"
/// - alpha/staging/etc: "Crate {Environment}"
fn get_app_name() -> String {
    let environment = option_env!("CRATE_ENV").unwrap_or("development");
    if environment == "production" {
        "Crate".to_string()
    } else {
        // Capitalize first letter
        let capitalized = environment
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default()
            + &environment[1..];
        format!("Crate {capitalized}")
    }
}

/// Menu translations from the frontend
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuTranslations {
    // Menu titles
    pub file: String,
    pub edit: String,
    pub playback: String,
    pub view: String,
    pub window: String,
    pub help: String,
    // App menu items (about and quit use {appName} placeholder, formatted by frontend)
    pub about: String,
    pub settings: String,
    pub quit: String,
    // File menu items
    pub import_tracks: String,
    pub new_playlist: String,
    pub new_folder: String,
    // Edit menu items
    pub undo: String,
    pub redo: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub select_all_tracks: String,
    // Playback menu items
    pub play_pause: String,
    pub stop: String,
    // View menu items
    pub toggle_sidebar: String,
    pub show_dev_tools: String,
    // Window menu items
    pub minimize: String,
    pub zoom: String,
    // Help menu items
    pub documentation: String,
    pub report_issue: String,
}

/// Menu item identifiers for event handling
pub mod ids {
    // Submenu IDs (for in-place translation updates)
    pub const APP_MENU: &str = "app_menu";
    pub const FILE_MENU: &str = "file_menu";
    pub const EDIT_MENU: &str = "edit_menu";
    pub const PLAYBACK_MENU: &str = "playback_menu";
    pub const VIEW_MENU: &str = "view_menu";
    pub const WINDOW_MENU: &str = "window_menu";
    pub const HELP_MENU: &str = "help_menu";

    // App menu items
    pub const ABOUT: &str = "about";
    pub const SETTINGS: &str = "settings";
    pub const QUIT: &str = "quit";

    // File menu items
    pub const IMPORT_TRACKS: &str = "import_tracks";
    pub const NEW_PLAYLIST: &str = "new_playlist";
    pub const NEW_FOLDER: &str = "new_folder";

    // Edit menu items
    pub const UNDO: &str = "undo";
    pub const REDO: &str = "redo";
    pub const CUT: &str = "cut";
    pub const COPY: &str = "copy";
    pub const PASTE: &str = "paste";
    pub const SELECT_ALL: &str = "select_all";

    // Playback menu items
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STOP: &str = "stop";

    // View menu items
    pub const TOGGLE_SIDEBAR: &str = "toggle_sidebar";
    pub const SHOW_DEVTOOLS: &str = "show_devtools";

    // Window menu items
    pub const MINIMIZE: &str = "minimize";
    pub const ZOOM: &str = "zoom";

    // Help menu items
    pub const DOCUMENTATION: &str = "documentation";
    pub const REPORT_ISSUE: &str = "report_issue";
}

/// Build the application menu
pub fn build_menu(app: &AppHandle<Wry>) -> Result<Menu<Wry>, tauri::Error> {
    let is_dev = option_env!("CRATE_ENV").map_or(true, |env| env != "production");

    let app_menu = build_app_menu(app)?;
    let file_menu = build_file_menu(app)?;
    let edit_menu = build_edit_menu(app)?;
    let playback_menu = build_playback_menu(app)?;
    let view_menu = build_view_menu(app, is_dev)?;
    let window_menu = build_window_menu(app)?;
    let help_menu = build_help_menu(app)?;

    MenuBuilder::new(app)
        .items(&[
            &app_menu,
            &file_menu,
            &edit_menu,
            &playback_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ])
        .build()
}

fn build_app_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    let app_name = get_app_name();
    SubmenuBuilder::with_id(app, ids::APP_MENU, &app_name)
        .item(&MenuItem::with_id(
            app,
            ids::ABOUT,
            format!("About {app_name}"),
            true,
            None::<&str>,
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS,
            "Settings...",
            true,
            Some("CmdOrCtrl+,"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::QUIT,
            format!("Quit {app_name}"),
            true,
            Some("CmdOrCtrl+Q"),
        )?)
        .build()
}

fn build_file_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::with_id(app, ids::FILE_MENU, "File")
        .item(&MenuItem::with_id(
            app,
            ids::IMPORT_TRACKS,
            "Import Tracks...",
            true,
            Some("CmdOrCtrl+O"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::NEW_PLAYLIST,
            "New Playlist",
            true,
            Some("CmdOrCtrl+N"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::NEW_FOLDER,
            "New Folder",
            true,
            Some("CmdOrCtrl+Shift+N"),
        )?)
        .build()
}

fn build_edit_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::with_id(app, ids::EDIT_MENU, "Edit")
        .item(&MenuItem::with_id(
            app,
            ids::UNDO,
            "Undo",
            true,
            Some("CmdOrCtrl+Z"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::REDO,
            "Redo",
            true,
            Some("CmdOrCtrl+Shift+Z"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::CUT,
            "Cut",
            true,
            Some("CmdOrCtrl+X"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::COPY,
            "Copy",
            true,
            Some("CmdOrCtrl+C"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::PASTE,
            "Paste",
            true,
            Some("CmdOrCtrl+V"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::SELECT_ALL,
            "Select All Tracks",
            true,
            Some("CmdOrCtrl+A"),
        )?)
        .build()
}

fn build_playback_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::with_id(app, ids::PLAYBACK_MENU, "Playback")
        .item(&MenuItem::with_id(
            app,
            ids::PLAY_PAUSE,
            "Play/Pause",
            true,
            Some("Space"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::STOP,
            "Stop",
            true,
            Some("CmdOrCtrl+."),
        )?)
        .build()
}

fn build_view_menu(app: &AppHandle<Wry>, is_dev: bool) -> Result<Submenu<Wry>, tauri::Error> {
    let mut builder = SubmenuBuilder::with_id(app, ids::VIEW_MENU, "View").item(&MenuItem::with_id(
        app,
        ids::TOGGLE_SIDEBAR,
        "Toggle Sidebar",
        true,
        Some("CmdOrCtrl+\\"),
    )?);

    if is_dev {
        builder = builder.separator().item(&MenuItem::with_id(
            app,
            ids::SHOW_DEVTOOLS,
            "Show DevTools",
            true,
            Some("CmdOrCtrl+Alt+I"),
        )?);
    }

    builder.build()
}

fn build_window_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::with_id(app, ids::WINDOW_MENU, "Window")
        .item(&MenuItem::with_id(
            app,
            ids::MINIMIZE,
            "Minimize",
            true,
            Some("CmdOrCtrl+M"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::ZOOM,
            "Zoom",
            true,
            None::<&str>,
        )?)
        .build()
}

fn build_help_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    let app_name = get_app_name();
    SubmenuBuilder::with_id(app, ids::HELP_MENU, "Help")
        .item(&MenuItem::with_id(
            app,
            ids::DOCUMENTATION,
            format!("{app_name} Documentation"),
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::REPORT_ISSUE,
            "Report an Issue...",
            true,
            None::<&str>,
        )?)
        .build()
}

/// Set up menu event handlers
pub fn setup_menu_handlers(app: &AppHandle<Wry>) {
    app.on_menu_event(move |app, event| {
        let id = event.id().0.as_str();

        // Handle backend-only actions
        match id {
            ids::QUIT => {
                app.exit(0);
                return;
            }
            ids::MINIMIZE => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.minimize();
                }
                return;
            }
            ids::ZOOM => {
                if let Some(window) = app.get_webview_window("main") {
                    // Toggle between maximized and normal state
                    if window.is_maximized().unwrap_or(false) {
                        let _ = window.unmaximize();
                    } else {
                        let _ = window.maximize();
                    }
                }
                return;
            }
            #[cfg(feature = "devtools")]
            ids::SHOW_DEVTOOLS => {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
                return;
            }
            _ => {}
        }

        // Emit event to frontend for all other actions
        if let Err(e) = app.emit("menu-action", id) {
            log::error!("Failed to emit menu event: {e}");
        }
    });
}

/// Update existing menu items with translated text in-place
/// This is preferred over rebuilding the entire menu as it works better on macOS
pub fn update_menu_translations(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<(), tauri::Error> {
    let Some(menu) = app.menu() else {
        return Ok(());
    };

    let is_dev = option_env!("CRATE_ENV").map_or(true, |env| env != "production");

    // Update submenu titles (note: App menu title is not translatable - it's the app name)
    update_submenu_text(&menu, ids::FILE_MENU, &translations.file)?;
    update_submenu_text(&menu, ids::EDIT_MENU, &translations.edit)?;
    update_submenu_text(&menu, ids::PLAYBACK_MENU, &translations.playback)?;
    update_submenu_text(&menu, ids::VIEW_MENU, &translations.view)?;
    update_submenu_text(&menu, ids::WINDOW_MENU, &translations.window)?;
    update_submenu_text(&menu, ids::HELP_MENU, &translations.help)?;

    // Update App menu items
    update_item_text(&menu, ids::ABOUT, &translations.about)?;
    update_item_text(&menu, ids::SETTINGS, &translations.settings)?;
    update_item_text(&menu, ids::QUIT, &translations.quit)?;

    // Update File menu items
    update_item_text(&menu, ids::IMPORT_TRACKS, &translations.import_tracks)?;
    update_item_text(&menu, ids::NEW_PLAYLIST, &translations.new_playlist)?;
    update_item_text(&menu, ids::NEW_FOLDER, &translations.new_folder)?;

    // Update Edit menu items
    update_item_text(&menu, ids::UNDO, &translations.undo)?;
    update_item_text(&menu, ids::REDO, &translations.redo)?;
    update_item_text(&menu, ids::CUT, &translations.cut)?;
    update_item_text(&menu, ids::COPY, &translations.copy)?;
    update_item_text(&menu, ids::PASTE, &translations.paste)?;
    update_item_text(&menu, ids::SELECT_ALL, &translations.select_all_tracks)?;

    // Update Playback menu items
    update_item_text(&menu, ids::PLAY_PAUSE, &translations.play_pause)?;
    update_item_text(&menu, ids::STOP, &translations.stop)?;

    // Update View menu items
    update_item_text(&menu, ids::TOGGLE_SIDEBAR, &translations.toggle_sidebar)?;
    if is_dev {
        update_item_text(&menu, ids::SHOW_DEVTOOLS, &translations.show_dev_tools)?;
    }

    // Update Window menu items
    update_item_text(&menu, ids::MINIMIZE, &translations.minimize)?;
    update_item_text(&menu, ids::ZOOM, &translations.zoom)?;

    // Update Help menu items
    update_item_text(&menu, ids::DOCUMENTATION, &translations.documentation)?;
    update_item_text(&menu, ids::REPORT_ISSUE, &translations.report_issue)?;

    Ok(())
}

fn update_submenu_text(menu: &Menu<Wry>, id: &str, text: &str) -> Result<(), tauri::Error> {
    if let Some(item) = menu.get(id) {
        if let MenuItemKind::Submenu(submenu) = item {
            submenu.set_text(text)?;
        }
    }
    Ok(())
}

fn update_item_text(menu: &Menu<Wry>, id: &str, text: &str) -> Result<(), tauri::Error> {
    // Menu items are nested inside submenus, so we need to search within each submenu
    if let Ok(items) = menu.items() {
        for item in items {
            if let MenuItemKind::Submenu(submenu) = item {
                if let Some(found) = submenu.get(id) {
                    if let MenuItemKind::MenuItem(menu_item) = found {
                        menu_item.set_text(text)?;
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}
