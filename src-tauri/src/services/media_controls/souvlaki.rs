//! Desktop media-session backend (souvlaki): OS Now Playing + hardware/media-key events.
//!
//! Hardware media keys arrive as souvlaki [`MediaControlEvent`]s and are re-emitted as the shared
//! `media-*` Tauri events, matching the iOS backend so the frontend handler is platform-agnostic.

use std::sync::Mutex;
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use tauri::{AppHandle, Emitter};

use super::{MediaSession, NowPlayingMetadata, PlaybackStatus};

pub struct SouvlakiMediaSession {
    controls: Option<Mutex<MediaControls>>,
}

impl SouvlakiMediaSession {
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
}

impl MediaSession for SouvlakiMediaSession {
    fn set_metadata(&self, meta: &NowPlayingMetadata) {
        let Some(ref controls) = self.controls else {
            return;
        };
        let Ok(mut controls) = controls.lock() else {
            return;
        };
        let _ = controls.set_metadata(MediaMetadata {
            title: meta.title.as_deref(),
            artist: meta.artist.as_deref(),
            album: meta.album.as_deref(),
            cover_url: meta.cover_url.as_deref(),
            duration: meta.duration,
        });
    }

    fn set_playback(&self, status: PlaybackStatus, progress: Option<Duration>) {
        let Some(ref controls) = self.controls else {
            return;
        };
        let Ok(mut controls) = controls.lock() else {
            return;
        };
        let progress = progress.map(MediaPosition);
        let playback = match status {
            PlaybackStatus::Playing => MediaPlayback::Playing { progress },
            PlaybackStatus::Paused => MediaPlayback::Paused { progress },
            PlaybackStatus::Stopped => MediaPlayback::Stopped,
        };
        let _ = controls.set_playback(playback);
    }

    fn clear(&self) {
        self.set_playback(PlaybackStatus::Stopped, None);
        self.set_metadata(&NowPlayingMetadata::default());
    }
}
