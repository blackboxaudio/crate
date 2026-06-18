//! iOS media-session backend + native lock-screen playback engine (objc2 FFI).
//!
//! Two concerns live here:
//!
//! 1. [`IosMediaSession`] — the [`MediaSession`](super::MediaSession) trait backend. On iOS the
//!    metadata methods are intentionally no-ops: discovery preview is driven by the native engine
//!    below (which owns `MPNowPlayingInfoCenter` directly), and the `update_now_playing` IPC isn't
//!    used on mobile. Its one job is activating `AVAudioSession.playback` for background audio (#79).
//!
//! 2. [`NativePreviewEngine`] (#54) — a native `AVPlayer` playback engine driving
//!    `MPNowPlayingInfoCenter` + `MPRemoteCommandCenter`. This replaces the WebView HTML5 `<audio>`
//!    path on iOS so the lock screen gets real transport (prev/next/scrubber) that keeps working
//!    while the WebView's JavaScript is suspended on lock. Registered in Tauri state and driven by the
//!    `native_preview_*` commands; pushes state back to the frontend via `native-preview-*` events.
//!
//! objc2 note: the framework `features` in `Cargo.toml` and the exact generated method names are
//! reconciled on device with `cargo check --target aarch64-apple-ios` (de-risking step #1).

use std::time::Duration;

use tauri::AppHandle;

use super::{MediaSession, NowPlayingMetadata, PlaybackStatus};

mod audio_session;
mod engine;
mod now_playing;
mod observers;
mod player;
mod remote_command;

pub use engine::{NativePreviewEngine, NativeTrackEntry};

pub struct IosMediaSession {
    // The native preview engine (see `engine`) owns the lock-screen surface, so this backend holds no
    // state beyond having activated the audio session in `new`.
}

impl IosMediaSession {
    pub fn new(_app_handle: &AppHandle) -> Self {
        audio_session::configure_playback_audio_session();
        Self {}
    }
}

impl MediaSession for IosMediaSession {
    // Now Playing on iOS is owned by the native preview engine (it has the real position/rate), so
    // these trait methods are deliberately inert — the cross-platform `update_now_playing` IPC isn't
    // part of the mobile preview path.
    fn set_metadata(&self, _meta: &NowPlayingMetadata) {}
    fn set_playback(&self, _status: PlaybackStatus, _progress: Option<Duration>) {}
    fn clear(&self) {}
}
