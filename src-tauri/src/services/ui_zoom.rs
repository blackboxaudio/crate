//! Desktop webview page zoom (Settings → Appearance, View → Zoom In / Zoom Out / Actual Size).
//!
//! The backend owns the zoom level: the native menu handles its own items here (like
//! Minimize / Window → Zoom) so the shortcuts work during onboarding and while the Settings
//! modal is open, and the frontend store only mirrors the value via `ui-zoom-changed`.
//! The level is persisted as a device-local setting (never cloud-synced — it depends on the
//! display) and re-applied from `setup()` so the window opens already zoomed.
//!
//! Desktop-only at runtime, but the ladder helpers are pure and unit-tested flagless.

use crate::error::Result;

/// Discrete zoom ladder: small steps around 100%, wider at the ends.
pub const UI_ZOOM_LEVELS: &[f64] = &[0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5];
pub const DEFAULT_UI_ZOOM: f64 = 1.0;
pub const SETTING_KEY: &str = "ui_zoom";

/// Snap an arbitrary level to the nearest ladder value (non-finite → default).
pub fn snap(level: f64) -> f64 {
    if !level.is_finite() {
        return DEFAULT_UI_ZOOM;
    }
    UI_ZOOM_LEVELS
        .iter()
        .copied()
        .min_by(|a, b| (a - level).abs().total_cmp(&(b - level).abs()))
        .unwrap_or(DEFAULT_UI_ZOOM)
}

/// Move `delta` rungs up or down the ladder from `current`, clamped at both ends.
pub fn step(current: f64, delta: i32) -> f64 {
    let snapped = snap(current);
    let idx = UI_ZOOM_LEVELS
        .iter()
        .position(|&l| l == snapped)
        .unwrap_or(0) as i32;
    let next = (idx + delta).clamp(0, UI_ZOOM_LEVELS.len() as i32 - 1);
    UI_ZOOM_LEVELS[next as usize]
}

/// Parse the stored setting value; garbage or a missing key falls back to the default.
pub fn parse_setting(raw: Option<String>) -> f64 {
    raw.and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map(snap)
        .unwrap_or(DEFAULT_UI_ZOOM)
}

/// WKWebView's `pageZoom` only exists on macOS 11+, but the app still installs on 10.13.
/// Calling it there raises an unrecognized-selector exception, so gate every `set_zoom`.
#[cfg(all(feature = "desktop", target_os = "macos"))]
fn page_zoom_supported() -> bool {
    sysinfo::System::os_version()
        .and_then(|v| v.split('.').next()?.parse::<u32>().ok())
        .map(|major| major >= 11)
        .unwrap_or(true)
}

#[cfg(all(feature = "desktop", not(target_os = "macos")))]
fn page_zoom_supported() -> bool {
    true
}

#[cfg(feature = "desktop")]
fn set_window_zoom(app: &tauri::AppHandle, level: f64) -> Result<()> {
    use crate::error::CrateError;
    use tauri::Manager;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| CrateError::InvalidOperation("Main window not found".to_string()))?;
    window
        .set_zoom(level)
        .map_err(|e| CrateError::InvalidOperation(format!("Failed to set webview zoom: {e}")))
}

/// Re-apply the persisted level at launch. Best-effort: never fails setup, and skipped at
/// 100% so installs that never touched zoom don't call into `pageZoom` at all.
#[cfg(feature = "desktop")]
pub fn apply_startup(app: &tauri::AppHandle, level: f64) {
    let level = snap(level);
    if level == DEFAULT_UI_ZOOM || !page_zoom_supported() {
        return;
    }
    if let Err(e) = set_window_zoom(app, level) {
        log::warn!("Failed to apply startup UI zoom {level}: {e}");
    }
}

/// Snap, apply to the window, persist, and notify the frontend. Returns the applied level.
#[cfg(feature = "desktop")]
pub fn apply(app: &tauri::AppHandle, level: f64) -> Result<f64> {
    use crate::services::SettingsService;
    use tauri::{Emitter, Manager};

    let settings = app.state::<SettingsService>();
    let level = snap(level);

    if !page_zoom_supported() {
        log::warn!("UI zoom requires macOS 11+; ignoring request for {level}");
        return Ok(settings.get_settings()?.ui_zoom);
    }

    set_window_zoom(app, level)?;
    settings.set_setting(SETTING_KEY, &level.to_string())?;
    if let Err(e) = app.emit("ui-zoom-changed", level) {
        log::error!("Failed to emit ui-zoom-changed: {e}");
    }
    Ok(level)
}

/// Step relative to the persisted level (menu Zoom In / Zoom Out).
#[cfg(feature = "desktop")]
pub fn step_and_apply(app: &tauri::AppHandle, delta: i32) -> Result<f64> {
    use crate::services::SettingsService;
    use tauri::Manager;

    let current = app.state::<SettingsService>().get_settings()?.ui_zoom;
    apply(app, step(current, delta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_clamps_and_rounds_to_ladder() {
        assert_eq!(snap(0.0), 0.75);
        assert_eq!(snap(2.0), 1.5);
        assert_eq!(snap(1.05), 1.0);
        assert_eq!(snap(1.2), 1.25);
        assert_eq!(snap(1.25), 1.25);
        assert_eq!(snap(f64::NAN), 1.0);
        assert_eq!(snap(f64::INFINITY), 1.0);
    }

    #[test]
    fn step_walks_ladder_and_clamps_at_ends() {
        assert_eq!(step(1.0, 1), 1.1);
        assert_eq!(step(1.0, -1), 0.9);
        assert_eq!(step(1.5, 1), 1.5);
        assert_eq!(step(0.75, -1), 0.75);
        assert_eq!(step(1.02, 1), 1.1);
        assert_eq!(step(1.0, 100), 1.5);
        assert_eq!(step(1.0, -100), 0.75);
        assert_eq!(step(1.0, 0), 1.0);
    }

    #[test]
    fn parse_setting_falls_back_to_default() {
        assert_eq!(parse_setting(None), 1.0);
        assert_eq!(parse_setting(Some("abc".to_string())), 1.0);
        assert_eq!(parse_setting(Some("NaN".to_string())), 1.0);
        assert_eq!(parse_setting(Some("1.25".to_string())), 1.25);
        assert_eq!(parse_setting(Some(" 0.8 ".to_string())), 0.8);
        assert_eq!(parse_setting(Some("3".to_string())), 1.5);
    }
}
