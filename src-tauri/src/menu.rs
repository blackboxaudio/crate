use tauri::{
    menu::{
        AboutMetadata, Menu, MenuBuilder, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder,
    },
    AppHandle, Emitter, Manager, Wry,
};

/// Menu item identifiers for event handling
pub mod ids {
    // File menu
    pub const IMPORT_TRACKS: &str = "import_tracks";
    pub const NEW_PLAYLIST: &str = "new_playlist";
    pub const NEW_FOLDER: &str = "new_folder";

    // Edit menu
    pub const SELECT_ALL: &str = "select_all";
    pub const DESELECT_ALL: &str = "deselect_all";

    // Playback menu
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STOP: &str = "stop";

    // View menu
    pub const TOGGLE_SIDEBAR: &str = "toggle_sidebar";
    pub const SHOW_DEVTOOLS: &str = "show_devtools";

    // Settings (from app menu)
    pub const SETTINGS: &str = "settings";

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
    SubmenuBuilder::new(app, "Crate")
        .about(Some(AboutMetadata {
            name: Some("Crate".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..Default::default()
        }))
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS,
            "Settings...",
            true,
            Some("CmdOrCtrl+,"),
        )?)
        .separator()
        .quit()
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
            ids::SELECT_ALL,
            "Select All",
            true,
            Some("CmdOrCtrl+A"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::DESELECT_ALL,
            "Deselect All",
            true,
            Some("CmdOrCtrl+Shift+A"),
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
        .minimize()
        .item(&PredefinedMenuItem::maximize(app, Some("Zoom"))?)
        .build()
}

fn build_help_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::new(app, "Help")
        .item(&MenuItem::with_id(
            app,
            ids::DOCUMENTATION,
            "Crate Documentation",
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
        #[cfg(feature = "devtools")]
        if id == ids::SHOW_DEVTOOLS {
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            return;
        }

        // Emit event to frontend for all other actions
        if let Err(e) = app.emit("menu-action", id) {
            log::error!("Failed to emit menu event: {e}");
        }
    });
}
