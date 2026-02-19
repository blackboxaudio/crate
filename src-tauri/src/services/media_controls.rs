use std::sync::Mutex;
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig,
};
use tauri::{AppHandle, Emitter};

pub struct MediaControlsService {
    controls: Option<Mutex<MediaControls>>,
}

impl MediaControlsService {
    pub fn new(app_handle: &AppHandle) -> Self {
        let config = PlatformConfig {
            dbus_name: "crate_app",
            display_name: "Crate",
            hwnd: None,
        };

        let mut controls = match MediaControls::new(config) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to initialize media controls: {e}");
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
