use std::sync::Mutex;
use std::time::Duration;

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use tauri::{AppHandle, Emitter};

pub struct MediaControlsService {
    controls: Option<Mutex<MediaControls>>,
}

impl MediaControlsService {
    pub fn new(app_handle: &AppHandle) -> Self {
        let hwnd = Self::get_hwnd(app_handle);

        // On Windows, souvlaki panics (via .expect()) if hwnd is None.
        // Skip initialization entirely rather than risk a crash.
        #[cfg(target_os = "windows")]
        if hwnd.is_none() {
            log::warn!("No window handle available; skipping media controls on Windows");
            return Self { controls: None };
        }

        let config = PlatformConfig {
            dbus_name: "crate_app",
            display_name: "Crate",
            hwnd,
        };

        // Wrap in catch_unwind as a safety net — souvlaki's Windows impl can panic
        // in ways that bypass its own Result return type.
        let controls_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| MediaControls::new(config)));

        let mut controls = match controls_result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                log::warn!("Failed to initialize media controls: {e}");
                return Self { controls: None };
            }
            Err(_) => {
                log::warn!("Media controls initialization panicked");
                return Self { controls: None };
            }
        };

        let handle = app_handle.clone();
        if let Err(e) = controls.attach(move |event: MediaControlEvent| {
            let event_name = match event {
                MediaControlEvent::Toggle => "media-toggle",
                MediaControlEvent::Play => "media-play",
                MediaControlEvent::Pause => "media-pause",
                MediaControlEvent::Next => "media-next",
                MediaControlEvent::Previous => "media-previous",
                _ => return,
            };
            let _ = handle.emit(event_name, ());
        }) {
            log::warn!("Failed to attach media controls handler: {e}");
            return Self { controls: None };
        }

        Self {
            controls: Some(Mutex::new(controls)),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_hwnd(app_handle: &AppHandle) -> Option<*mut std::ffi::c_void> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use tauri::Manager;

        let window = app_handle.get_webview_window("main")?;
        let handle = window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn get_hwnd(_app_handle: &AppHandle) -> Option<*mut std::ffi::c_void> {
        None
    }

    pub fn set_metadata(
        &self,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        cover_url: Option<&str>,
        duration: Option<Duration>,
    ) {
        let Some(ref controls) = self.controls else {
            return;
        };
        let Ok(mut controls) = controls.lock() else {
            return;
        };
        let _ = controls.set_metadata(MediaMetadata {
            title,
            artist,
            album,
            cover_url,
            duration,
        });
    }

    pub fn set_playback(&self, playback: MediaPlayback) {
        let Some(ref controls) = self.controls else {
            return;
        };
        let Ok(mut controls) = controls.lock() else {
            return;
        };
        let _ = controls.set_playback(playback);
    }
}
