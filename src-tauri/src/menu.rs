use serde::Deserialize;
use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, Submenu, SubmenuBuilder},
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
        format!("Crate {}", capitalized)
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
    // App menu
    pub const ABOUT: &str = "about";
    pub const SETTINGS: &str = "settings";
    pub const QUIT: &str = "quit";

    // File menu
    pub const IMPORT_TRACKS: &str = "import_tracks";
    pub const NEW_PLAYLIST: &str = "new_playlist";
    pub const NEW_FOLDER: &str = "new_folder";

    // Edit menu
    pub const UNDO: &str = "undo";
    pub const REDO: &str = "redo";
    pub const CUT: &str = "cut";
    pub const COPY: &str = "copy";
    pub const PASTE: &str = "paste";
    pub const SELECT_ALL: &str = "select_all";

    // Playback menu
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STOP: &str = "stop";

    // View menu
    pub const TOGGLE_SIDEBAR: &str = "toggle_sidebar";
    pub const SHOW_DEVTOOLS: &str = "show_devtools";

    // Window menu
    pub const MINIMIZE: &str = "minimize";
    pub const ZOOM: &str = "zoom";

    // Help menu
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
    SubmenuBuilder::new(app, &app_name)
        .item(&MenuItem::with_id(
            app,
            ids::ABOUT,
            format!("About {}", app_name),
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
            format!("Quit {}", app_name),
            true,
            Some("CmdOrCtrl+Q"),
        )?)
        .build()
}

fn build_file_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, "File")
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
    SubmenuBuilder::new(app, "Edit")
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
    SubmenuBuilder::new(app, "Playback")
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
    let mut builder = SubmenuBuilder::new(app, "View").item(&MenuItem::with_id(
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
    SubmenuBuilder::new(app, "Window")
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
    SubmenuBuilder::new(app, "Help")
        .item(&MenuItem::with_id(
            app,
            ids::DOCUMENTATION,
            format!("{} Documentation", app_name),
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

/// Rebuild the menu with translated labels
pub fn rebuild_menu_with_translations(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Menu<Wry>, tauri::Error> {
    let is_dev = option_env!("CRATE_ENV").map_or(true, |env| env != "production");

    let app_menu = build_app_menu_translated(app, translations)?;
    let file_menu = build_file_menu_translated(app, translations)?;
    let edit_menu = build_edit_menu_translated(app, translations)?;
    let playback_menu = build_playback_menu_translated(app, translations)?;
    let view_menu = build_view_menu_translated(app, translations, is_dev)?;
    let window_menu = build_window_menu_translated(app, translations)?;
    let help_menu = build_help_menu_translated(app, translations)?;

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

fn build_app_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Submenu<Wry>, tauri::Error> {
    let app_name = get_app_name();
    SubmenuBuilder::new(app, &app_name)
        .item(&MenuItem::with_id(
            app,
            ids::ABOUT,
            &translations.about,
            true,
            None::<&str>,
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS,
            &translations.settings,
            true,
            Some("CmdOrCtrl+,"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::QUIT,
            &translations.quit,
            true,
            Some("CmdOrCtrl+Q"),
        )?)
        .build()
}

fn build_file_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, &translations.file)
        .item(&MenuItem::with_id(
            app,
            ids::IMPORT_TRACKS,
            &translations.import_tracks,
            true,
            Some("CmdOrCtrl+O"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::NEW_PLAYLIST,
            &translations.new_playlist,
            true,
            Some("CmdOrCtrl+N"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::NEW_FOLDER,
            &translations.new_folder,
            true,
            Some("CmdOrCtrl+Shift+N"),
        )?)
        .build()
}

fn build_edit_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, &translations.edit)
        .item(&MenuItem::with_id(
            app,
            ids::UNDO,
            &translations.undo,
            true,
            Some("CmdOrCtrl+Z"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::REDO,
            &translations.redo,
            true,
            Some("CmdOrCtrl+Shift+Z"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::CUT,
            &translations.cut,
            true,
            Some("CmdOrCtrl+X"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::COPY,
            &translations.copy,
            true,
            Some("CmdOrCtrl+C"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::PASTE,
            &translations.paste,
            true,
            Some("CmdOrCtrl+V"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::SELECT_ALL,
            &translations.select_all_tracks,
            true,
            Some("CmdOrCtrl+A"),
        )?)
        .build()
}

fn build_playback_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, &translations.playback)
        .item(&MenuItem::with_id(
            app,
            ids::PLAY_PAUSE,
            &translations.play_pause,
            true,
            Some("Space"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::STOP,
            &translations.stop,
            true,
            Some("CmdOrCtrl+."),
        )?)
        .build()
}

fn build_view_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
    is_dev: bool,
) -> Result<Submenu<Wry>, tauri::Error> {
    let mut builder = SubmenuBuilder::new(app, &translations.view).item(&MenuItem::with_id(
        app,
        ids::TOGGLE_SIDEBAR,
        &translations.toggle_sidebar,
        true,
        Some("CmdOrCtrl+\\"),
    )?);

    if is_dev {
        builder = builder.separator().item(&MenuItem::with_id(
            app,
            ids::SHOW_DEVTOOLS,
            &translations.show_dev_tools,
            true,
            Some("CmdOrCtrl+Alt+I"),
        )?);
    }

    builder.build()
}

fn build_window_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, &translations.window)
        .item(&MenuItem::with_id(
            app,
            ids::MINIMIZE,
            &translations.minimize,
            true,
            Some("CmdOrCtrl+M"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::ZOOM,
            &translations.zoom,
            true,
            None::<&str>,
        )?)
        .build()
}

fn build_help_menu_translated(
    app: &AppHandle<Wry>,
    translations: &MenuTranslations,
) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, &translations.help)
        .item(&MenuItem::with_id(
            app,
            ids::DOCUMENTATION,
            &translations.documentation,
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::REPORT_ISSUE,
            &translations.report_issue,
            true,
            None::<&str>,
        )?)
        .build()
}
