use std::sync::Mutex;

use serde::Deserialize;
use tauri::{
    menu::{
        Menu, MenuBuilder, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu, SubmenuBuilder,
    },
    AppHandle, Emitter, Manager, Wry,
};

/// Cached fullscreen menu labels for dynamic text toggling.
/// Stored as Tauri state so the backend can update the menu text
/// when the window enters/exits fullscreen without needing the frontend.
pub struct FullscreenLabels {
    pub enter: String,
    pub exit: String,
}

impl Default for FullscreenLabels {
    fn default() -> Self {
        Self {
            enter: "Enter Full Screen".to_string(),
            exit: "Exit Full Screen".to_string(),
        }
    }
}

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
    pub add_release: String,
    pub refresh_metadata: String,
    pub new_playlist: String,
    pub new_folder: String,
    pub quick_export: String,
    // Edit menu items
    pub undo: String,
    pub redo: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub select_all: String,
    // Playback menu items
    pub play_pause: String,
    pub stop: String,
    pub next_track: String,
    pub previous_track: String,
    pub seek_forward: String,
    pub seek_backward: String,
    pub fine_seek_forward: String,
    pub fine_seek_backward: String,
    pub volume_up: String,
    pub volume_down: String,
    pub mute: String,
    pub jump_to_playing: String,
    // View menu items
    pub toggle_view: String,
    pub toggle_editor: String,
    pub expand_all_releases: String,
    pub collapse_all_releases: String,
    pub show_dev_tools: String,
    // Settings submenu
    pub settings_submenu: String,
    pub settings_general: String,
    pub settings_discovery: String,
    pub settings_library: String,
    pub settings_appearance: String,
    pub settings_sound: String,
    pub settings_diagnostics: String,
    // View menu items (predefined)
    pub enter_full_screen: String,
    pub exit_full_screen: String,
    // Window menu items
    pub minimize: String,
    pub zoom: String,
    // Help menu items
    pub feature_tour: String,
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
    pub const ADD_RELEASE: &str = "add_release";
    pub const REFRESH_METADATA: &str = "refresh_metadata";
    pub const NEW_PLAYLIST: &str = "new_playlist";
    pub const NEW_FOLDER: &str = "new_folder";
    pub const QUICK_EXPORT: &str = "quick_export";

    // Playback menu items
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STOP: &str = "stop";
    pub const NEXT_TRACK: &str = "next_track";
    pub const PREVIOUS_TRACK: &str = "previous_track";
    pub const SEEK_FORWARD: &str = "seek_forward";
    pub const SEEK_BACKWARD: &str = "seek_backward";
    pub const FINE_SEEK_FORWARD: &str = "fine_seek_forward";
    pub const FINE_SEEK_BACKWARD: &str = "fine_seek_backward";
    pub const VOLUME_UP: &str = "volume_up";
    pub const VOLUME_DOWN: &str = "volume_down";
    pub const MUTE: &str = "mute";
    pub const JUMP_TO_PLAYING: &str = "jump_to_playing";

    // View menu items
    pub const TOGGLE_VIEW: &str = "toggle_view";
    pub const TOGGLE_EDITOR: &str = "toggle_editor";
    pub const EXPAND_ALL_RELEASES: &str = "expand_all_releases";
    pub const COLLAPSE_ALL_RELEASES: &str = "collapse_all_releases";
    pub const SHOW_DEVTOOLS: &str = "show_devtools";

    // Settings submenu items
    pub const SETTINGS_MENU: &str = "settings_menu";
    pub const SETTINGS_GENERAL: &str = "settings_general";
    pub const SETTINGS_LIBRARY: &str = "settings_library";
    pub const SETTINGS_DISCOVERY: &str = "settings_discovery";
    pub const SETTINGS_APPEARANCE: &str = "settings_appearance";
    pub const SETTINGS_SOUND: &str = "settings_sound";
    pub const SETTINGS_DIAGNOSTICS: &str = "settings_diagnostics";

    // Window menu items
    pub const MINIMIZE: &str = "minimize";
    pub const ZOOM: &str = "zoom";

    // Help menu items
    pub const FEATURE_TOUR: &str = "feature_tour";
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
            Some("CmdOrCtrl+L"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::ADD_RELEASE,
            "Add Release...",
            true,
            Some("CmdOrCtrl+D"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::REFRESH_METADATA,
            "Refresh Metadata",
            false,
            Some("CmdOrCtrl+R"),
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
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::QUICK_EXPORT,
            "Quick Export...",
            true,
            Some("CmdOrCtrl+E"),
        )?)
        .build()
}

fn build_edit_menu(app: &AppHandle<Wry>) -> Result<Submenu<Wry>, tauri::Error> {
    SubmenuBuilder::with_id(app, ids::EDIT_MENU, "Edit")
        .item(&PredefinedMenuItem::undo(app, Some("Undo"))?)
        .item(&PredefinedMenuItem::redo(app, Some("Redo"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("Cut"))?)
        .item(&PredefinedMenuItem::copy(app, Some("Copy"))?)
        .item(&PredefinedMenuItem::paste(app, Some("Paste"))?)
        .separator()
        .item(&PredefinedMenuItem::select_all(app, Some("Select All"))?)
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
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::NEXT_TRACK,
            "Next Track",
            true,
            Some("Shift+Right"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::PREVIOUS_TRACK,
            "Previous Track",
            true,
            Some("Shift+Left"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::SEEK_FORWARD,
            "Seek Forward",
            true,
            Some("Right"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::SEEK_BACKWARD,
            "Seek Backward",
            true,
            Some("Left"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::FINE_SEEK_FORWARD,
            "Fine Seek Forward",
            true,
            Some("CmdOrCtrl+Right"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::FINE_SEEK_BACKWARD,
            "Fine Seek Backward",
            true,
            Some("CmdOrCtrl+Left"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::VOLUME_UP,
            "Volume Up",
            true,
            Some("Up"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::VOLUME_DOWN,
            "Volume Down",
            true,
            Some("Down"),
        )?)
        .item(&MenuItem::with_id(app, ids::MUTE, "Mute", true, Some("M"))?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::JUMP_TO_PLAYING,
            "Jump to Playing Track",
            true,
            Some("CmdOrCtrl+J"),
        )?)
        .build()
}

fn build_view_menu(app: &AppHandle<Wry>, is_dev: bool) -> Result<Submenu<Wry>, tauri::Error> {
    // Build the Settings submenu
    let settings_submenu = SubmenuBuilder::with_id(app, ids::SETTINGS_MENU, "Settings")
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS_GENERAL,
            "General",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS_APPEARANCE,
            "Appearance",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS_LIBRARY,
            "Library",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS_DISCOVERY,
            "Discovery",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS_SOUND,
            "Sound",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::SETTINGS_DIAGNOSTICS,
            "Diagnostics",
            true,
            None::<&str>,
        )?)
        .build()?;

    let mut builder = SubmenuBuilder::with_id(app, ids::VIEW_MENU, "View")
        .item(&MenuItem::with_id(
            app,
            ids::TOGGLE_VIEW,
            "Toggle View",
            true,
            Some("Shift+Tab"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::TOGGLE_EDITOR,
            "Toggle Editor",
            true,
            Some("CmdOrCtrl+I"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            ids::EXPAND_ALL_RELEASES,
            "Expand All Releases",
            true,
            Some("CmdOrCtrl+Shift+E"),
        )?)
        .item(&MenuItem::with_id(
            app,
            ids::COLLAPSE_ALL_RELEASES,
            "Collapse All Releases",
            true,
            Some("CmdOrCtrl+Shift+W"),
        )?)
        .separator()
        .item(&settings_submenu)
        .separator()
        .item(&PredefinedMenuItem::fullscreen(
            app,
            Some("Enter Full Screen"),
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
    SubmenuBuilder::with_id(app, ids::HELP_MENU, "Help")
        .item(&MenuItem::with_id(
            app,
            ids::FEATURE_TOUR,
            "Feature Tour...",
            true,
            None::<&str>,
        )?)
        .separator()
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

/// Menu items with keyboard accelerators that conflict with native dialog navigation.
/// These should be disabled while a native file dialog is open.
const DIALOG_CONFLICTING_ITEMS: &[&str] = &[
    ids::PLAY_PAUSE,
    ids::STOP,
    ids::NEXT_TRACK,
    ids::PREVIOUS_TRACK,
    ids::SEEK_FORWARD,
    ids::SEEK_BACKWARD,
    ids::FINE_SEEK_FORWARD,
    ids::FINE_SEEK_BACKWARD,
    ids::VOLUME_UP,
    ids::VOLUME_DOWN,
    ids::MUTE,
    ids::JUMP_TO_PLAYING,
    ids::TOGGLE_VIEW,
];

/// Set the enabled state of all menu items whose keyboard accelerators
/// conflict with native file dialog navigation (arrows, Space, M, etc.)
pub fn set_dialog_conflicting_items_enabled(
    app: &AppHandle<Wry>,
    enabled: bool,
) -> Result<(), tauri::Error> {
    for id in DIALOG_CONFLICTING_ITEMS {
        set_menu_item_enabled(app, id, enabled)?;
    }
    Ok(())
}

/// Menu items that should be disabled during the onboarding wizard.
/// These items are unsafe or meaningless before the app is fully initialized.
const ONBOARDING_DISABLED_ITEMS: &[&str] = &[
    // App menu
    ids::SETTINGS,
    // File menu
    ids::IMPORT_TRACKS,
    ids::ADD_RELEASE,
    ids::REFRESH_METADATA,
    ids::NEW_PLAYLIST,
    ids::NEW_FOLDER,
    ids::QUICK_EXPORT,
    // Playback menu
    ids::PLAY_PAUSE,
    ids::STOP,
    ids::NEXT_TRACK,
    ids::PREVIOUS_TRACK,
    ids::SEEK_FORWARD,
    ids::SEEK_BACKWARD,
    ids::FINE_SEEK_FORWARD,
    ids::FINE_SEEK_BACKWARD,
    ids::VOLUME_UP,
    ids::VOLUME_DOWN,
    ids::MUTE,
    ids::JUMP_TO_PLAYING,
    // View menu
    ids::TOGGLE_VIEW,
    ids::TOGGLE_EDITOR,
    ids::EXPAND_ALL_RELEASES,
    ids::COLLAPSE_ALL_RELEASES,
    // Help menu
    ids::FEATURE_TOUR,
    ids::REPORT_ISSUE,
];

/// Settings submenu items disabled during onboarding (nested inside View > Settings).
const ONBOARDING_DISABLED_NESTED_ITEMS: &[&str] = &[
    ids::SETTINGS_GENERAL,
    ids::SETTINGS_APPEARANCE,
    ids::SETTINGS_LIBRARY,
    ids::SETTINGS_DISCOVERY,
    ids::SETTINGS_SOUND,
    ids::SETTINGS_DIAGNOSTICS,
];

/// Set the enabled state of all menu items that should be disabled during onboarding.
pub fn set_onboarding_items_enabled(
    app: &AppHandle<Wry>,
    enabled: bool,
) -> Result<(), tauri::Error> {
    for id in ONBOARDING_DISABLED_ITEMS {
        set_menu_item_enabled(app, id, enabled)?;
    }

    // Handle nested settings submenu items (View > Settings > ...)
    if let Some(menu) = app.menu() {
        if let Some(MenuItemKind::Submenu(view_submenu)) = menu.get(ids::VIEW_MENU) {
            if let Some(MenuItemKind::Submenu(settings_submenu)) =
                view_submenu.get(ids::SETTINGS_MENU)
            {
                for id in ONBOARDING_DISABLED_NESTED_ITEMS {
                    if let Some(MenuItemKind::MenuItem(menu_item)) = settings_submenu.get(*id) {
                        menu_item.set_enabled(enabled)?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Set the enabled state of a menu item by its ID
pub fn set_menu_item_enabled(
    app: &AppHandle<Wry>,
    id: &str,
    enabled: bool,
) -> Result<(), tauri::Error> {
    if let Some(menu) = app.menu() {
        if let Ok(items) = menu.items() {
            for item in items {
                if let MenuItemKind::Submenu(submenu) = item {
                    if let Some(MenuItemKind::MenuItem(menu_item)) = submenu.get(id) {
                        menu_item.set_enabled(enabled)?;
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
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
    update_item_text(&menu, ids::ADD_RELEASE, &translations.add_release)?;
    update_item_text(&menu, ids::REFRESH_METADATA, &translations.refresh_metadata)?;
    update_item_text(&menu, ids::NEW_PLAYLIST, &translations.new_playlist)?;
    update_item_text(&menu, ids::NEW_FOLDER, &translations.new_folder)?;
    update_item_text(&menu, ids::QUICK_EXPORT, &translations.quick_export)?;

    // Update Edit menu items (PredefinedMenuItems by position)
    if let Some(MenuItemKind::Submenu(edit_submenu)) = menu.get(ids::EDIT_MENU) {
        let edit_texts: [&str; 6] = [
            &translations.undo,
            &translations.redo,
            &translations.cut,
            &translations.copy,
            &translations.paste,
            &translations.select_all,
        ];
        let predefined: Vec<_> = edit_submenu
            .items()?
            .into_iter()
            .filter_map(|item| match item {
                MenuItemKind::Predefined(p) => {
                    // Skip separators (they have empty text)
                    if p.text().unwrap_or_default().is_empty() {
                        return None;
                    }
                    Some(p)
                }
                _ => None,
            })
            .collect();
        for (item, text) in predefined.iter().zip(edit_texts.iter()) {
            item.set_text(*text)?;
        }
    }

    // Update Playback menu items
    update_item_text(&menu, ids::PLAY_PAUSE, &translations.play_pause)?;
    update_item_text(&menu, ids::STOP, &translations.stop)?;
    update_item_text(&menu, ids::NEXT_TRACK, &translations.next_track)?;
    update_item_text(&menu, ids::PREVIOUS_TRACK, &translations.previous_track)?;
    update_item_text(&menu, ids::SEEK_FORWARD, &translations.seek_forward)?;
    update_item_text(&menu, ids::SEEK_BACKWARD, &translations.seek_backward)?;
    update_item_text(
        &menu,
        ids::FINE_SEEK_FORWARD,
        &translations.fine_seek_forward,
    )?;
    update_item_text(
        &menu,
        ids::FINE_SEEK_BACKWARD,
        &translations.fine_seek_backward,
    )?;
    update_item_text(&menu, ids::VOLUME_UP, &translations.volume_up)?;
    update_item_text(&menu, ids::VOLUME_DOWN, &translations.volume_down)?;
    update_item_text(&menu, ids::MUTE, &translations.mute)?;
    update_item_text(&menu, ids::JUMP_TO_PLAYING, &translations.jump_to_playing)?;

    // Update View menu items
    update_item_text(&menu, ids::TOGGLE_VIEW, &translations.toggle_view)?;
    update_item_text(&menu, ids::TOGGLE_EDITOR, &translations.toggle_editor)?;
    update_item_text(
        &menu,
        ids::EXPAND_ALL_RELEASES,
        &translations.expand_all_releases,
    )?;
    update_item_text(
        &menu,
        ids::COLLAPSE_ALL_RELEASES,
        &translations.collapse_all_releases,
    )?;
    if is_dev {
        update_item_text(&menu, ids::SHOW_DEVTOOLS, &translations.show_dev_tools)?;
    }

    // Update Settings submenu (nested inside View menu)
    update_nested_submenu_text(
        &menu,
        ids::VIEW_MENU,
        ids::SETTINGS_MENU,
        &translations.settings_submenu,
    )?;
    update_nested_submenu_items(
        &menu,
        ids::VIEW_MENU,
        ids::SETTINGS_MENU,
        &[
            (ids::SETTINGS_GENERAL, &translations.settings_general),
            (ids::SETTINGS_APPEARANCE, &translations.settings_appearance),
            (ids::SETTINGS_DISCOVERY, &translations.settings_discovery),
            (ids::SETTINGS_LIBRARY, &translations.settings_library),
            (ids::SETTINGS_SOUND, &translations.settings_sound),
            (
                ids::SETTINGS_DIAGNOSTICS,
                &translations.settings_diagnostics,
            ),
        ],
    )?;

    // Update fullscreen labels in state and set correct text based on current window state
    if let Some(labels_state) = app.try_state::<Mutex<FullscreenLabels>>() {
        if let Ok(mut labels) = labels_state.lock() {
            labels.enter = translations.enter_full_screen.clone();
            labels.exit = translations.exit_full_screen.clone();
        }
    }
    let is_fullscreen = app
        .get_webview_window("main")
        .and_then(|w| w.is_fullscreen().ok())
        .unwrap_or(false);
    set_fullscreen_menu_text(&menu, is_fullscreen, app)?;

    // Update Window menu items
    update_item_text(&menu, ids::MINIMIZE, &translations.minimize)?;
    update_item_text(&menu, ids::ZOOM, &translations.zoom)?;

    // Update Help menu items
    update_item_text(&menu, ids::FEATURE_TOUR, &translations.feature_tour)?;
    update_item_text(&menu, ids::REPORT_ISSUE, &translations.report_issue)?;

    Ok(())
}

fn update_submenu_text(menu: &Menu<Wry>, id: &str, text: &str) -> Result<(), tauri::Error> {
    if let Some(MenuItemKind::Submenu(submenu)) = menu.get(id) {
        submenu.set_text(text)?;
    }
    Ok(())
}

fn update_item_text(menu: &Menu<Wry>, id: &str, text: &str) -> Result<(), tauri::Error> {
    // Menu items are nested inside submenus, so we need to search within each submenu
    if let Ok(items) = menu.items() {
        for item in items {
            if let MenuItemKind::Submenu(submenu) = item {
                if let Some(MenuItemKind::MenuItem(menu_item)) = submenu.get(id) {
                    menu_item.set_text(text)?;
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

/// Update the title of a nested submenu (e.g., View > Settings)
fn update_nested_submenu_text(
    menu: &Menu<Wry>,
    parent_submenu_id: &str,
    nested_submenu_id: &str,
    text: &str,
) -> Result<(), tauri::Error> {
    if let Some(MenuItemKind::Submenu(parent)) = menu.get(parent_submenu_id) {
        if let Some(MenuItemKind::Submenu(nested)) = parent.get(nested_submenu_id) {
            nested.set_text(text)?;
        }
    }
    Ok(())
}

/// Update menu items within a nested submenu (e.g., View > Settings > General)
fn update_nested_submenu_items(
    menu: &Menu<Wry>,
    parent_submenu_id: &str,
    nested_submenu_id: &str,
    items: &[(&str, &str)],
) -> Result<(), tauri::Error> {
    // Find the parent submenu (e.g., View menu)
    if let Some(MenuItemKind::Submenu(parent)) = menu.get(parent_submenu_id) {
        // Find the nested submenu (e.g., Settings submenu)
        if let Some(MenuItemKind::Submenu(nested)) = parent.get(nested_submenu_id) {
            // Update each menu item in the nested submenu
            for (item_id, text) in items {
                if let Some(MenuItemKind::MenuItem(menu_item)) = nested.get(*item_id) {
                    menu_item.set_text(*text)?;
                }
            }
        }
    }
    Ok(())
}

/// Update the fullscreen menu item text based on current fullscreen state.
/// Called from `on_window_event` when the fullscreen state changes.
pub fn update_fullscreen_menu_text(app: &AppHandle<Wry>, is_fullscreen: bool) {
    let Some(menu) = app.menu() else {
        return;
    };
    if let Err(e) = set_fullscreen_menu_text(&menu, is_fullscreen, app) {
        log::error!("Failed to update fullscreen menu text: {e}");
    }
}

/// Set the fullscreen PredefinedMenuItem text using cached labels from Tauri state.
fn set_fullscreen_menu_text(
    menu: &Menu<Wry>,
    is_fullscreen: bool,
    app: &AppHandle<Wry>,
) -> Result<(), tauri::Error> {
    let text = if let Some(labels_state) = app.try_state::<Mutex<FullscreenLabels>>() {
        let labels = labels_state.lock().unwrap_or_else(|e| e.into_inner());
        if is_fullscreen {
            labels.exit.clone()
        } else {
            labels.enter.clone()
        }
    } else {
        // Fallback before state is initialized
        if is_fullscreen {
            "Exit Full Screen".to_string()
        } else {
            "Enter Full Screen".to_string()
        }
    };

    if let Some(MenuItemKind::Submenu(view_submenu)) = menu.get(ids::VIEW_MENU) {
        let predefined: Vec<_> = view_submenu
            .items()?
            .into_iter()
            .filter_map(|item| match item {
                MenuItemKind::Predefined(p) => {
                    if p.text().unwrap_or_default().is_empty() {
                        return None;
                    }
                    Some(p)
                }
                _ => None,
            })
            .collect();
        if let Some(fullscreen) = predefined.first() {
            fullscreen.set_text(&text)?;
        }
    }
    Ok(())
}
