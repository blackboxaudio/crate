//! Cross-platform media-session abstraction ("Now Playing" / lock-screen controls).
//!
//! A single public service ([`MediaControlsService`]) wraps a platform-selected backend:
//! - **desktop** (`feature = "desktop"`): souvlaki — OS Now Playing + hardware media keys.
//! - **iOS** (`target_os = "ios"`): `AVAudioSession` + `MPNowPlayingInfoCenter` +
//!   `MPRemoteCommandCenter` (#79).
//! - **everything else** (Android, flagless host/test builds): a no-op.
//!
//! Every backend emits the same `media-toggle` / `media-play` / `media-pause` / `media-next` /
//! `media-previous` (and, on iOS, `media-seek`) Tauri events, so the frontend handler is shared.
//! Metadata flows in via the `update_now_playing` / `update_playback_state` / `clear_now_playing`
//! commands (frontend IPC), which call the methods below.

use std::time::Duration;

use tauri::AppHandle;

// Exactly one backend is selected by `MediaControlsService::new` below; gate each module so only
// the relevant one (and its native deps) compiles per platform.
#[cfg(target_os = "ios")]
mod ios;
#[cfg(not(any(all(feature = "desktop", not(target_os = "ios")), target_os = "ios")))]
mod noop;
#[cfg(all(feature = "desktop", not(target_os = "ios")))]
mod souvlaki;

// The native iOS preview-playback engine (#54): registered in Tauri state and driven by the
// `native_preview_*` commands.
#[cfg(target_os = "ios")]
pub use ios::{NativePreviewEngine, NativeTrackEntry};

/// Platform-neutral "Now Playing" metadata pushed to the OS lock screen / media UI.
///
/// On iOS the lock-screen surface is owned by the native preview engine (see the `ios` backend), so
/// this struct is populated by the `update_now_playing` IPC but never read by the iOS backend (a
/// no-op) — hence the iOS-only dead-code allow. Desktop (souvlaki) reads every field.
#[cfg_attr(target_os = "ios", allow(dead_code))]
#[derive(Debug, Default, Clone)]
pub struct NowPlayingMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Artwork as a `file://` path or an `http(s)://` URL.
    pub cover_url: Option<String>,
    pub duration: Option<Duration>,
}

/// Platform-neutral playback status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    // Only constructed by the desktop souvlaki `clear()`; the iOS no-op backend never builds it.
    #[cfg_attr(target_os = "ios", allow(dead_code))]
    Stopped,
}

/// A platform media-session backend. Implementations own the OS-level Now Playing surface and, where
/// applicable, emit `media-*` events for remote-control / interruption / route-change actions.
pub trait MediaSession: Send + Sync {
    /// Update the displayed track metadata.
    fn set_metadata(&self, meta: &NowPlayingMetadata);
    /// Update the playback status, with an optional elapsed position for the progress bar.
    fn set_playback(&self, status: PlaybackStatus, progress: Option<Duration>);
    /// Clear Now Playing (stop + empty metadata).
    fn clear(&self);
}

/// The single service registered in Tauri state on every platform. Delegates to a compile-time
/// selected backend; all methods are infallible no-ops where no native backend exists.
pub struct MediaControlsService {
    backend: Box<dyn MediaSession>,
}

impl MediaControlsService {
    pub fn new(app_handle: &AppHandle) -> Self {
        #[cfg(target_os = "ios")]
        let backend: Box<dyn MediaSession> = Box::new(ios::IosMediaSession::new(app_handle));
        #[cfg(all(feature = "desktop", not(target_os = "ios")))]
        let backend: Box<dyn MediaSession> =
            Box::new(souvlaki::SouvlakiMediaSession::new(app_handle));
        #[cfg(not(any(all(feature = "desktop", not(target_os = "ios")), target_os = "ios")))]
        let backend: Box<dyn MediaSession> = {
            let _ = app_handle;
            Box::new(noop::NoopMediaSession)
        };

        Self { backend }
    }

    pub fn set_metadata(&self, meta: &NowPlayingMetadata) {
        self.backend.set_metadata(meta);
    }

    pub fn set_playback(&self, status: PlaybackStatus, progress: Option<Duration>) {
        self.backend.set_playback(status, progress);
    }

    pub fn clear(&self) {
        self.backend.clear();
    }
}
