//! Native preview engine: the Send+Sync Tauri-state handle, main-thread dispatch, and the
//! backend→frontend events.

use std::cell::RefCell;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::player::PlaybackEngineInner;

// The AVPlayer-backed engine is not `Send` (objc objects are main-thread-bound), so it can't live in
// Send+Sync Tauri state directly. Instead it lives in a main-thread thread-local; the Tauri state
// ([`NativePreviewEngine`]) holds only the `AppHandle` and funnels every operation onto the main
// thread via `run_on_main_thread`, where this thread-local is the single instance ever touched.
thread_local! {
    static ENGINE: RefCell<Option<PlaybackEngineInner>> = const { RefCell::new(None) };
}

/// One pre-resolved track handed to the native engine. `url` is the localhost proxy URL
/// (`http://127.0.0.1:{port}/{release}/{position}`) the frontend already resolved via
/// `fetch_preview_stream`, so the engine can switch tracks — including while the screen is locked and
/// the WebView's JS is suspended — without any further resolution.
#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTrackEntry {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub artwork_url: Option<String>,
}

/// Send+Sync Tauri-state handle to the native playback engine. All real work happens on the main
/// thread against the [`ENGINE`] thread-local.
pub struct NativePreviewEngine {
    app: AppHandle,
}

impl NativePreviewEngine {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// Load a release's tracks (pre-resolved proxy URLs) and start playing from `start_index`.
    pub fn play(&self, tracks: Vec<NativeTrackEntry>, start_index: usize) {
        let app = self.app.clone();
        run_on_main(&self.app, move || {
            ENGINE.with(|cell| {
                let mut slot = cell.borrow_mut();
                let engine = slot.get_or_insert_with(|| PlaybackEngineInner::new(app.clone()));
                engine.load(tracks, start_index);
            });
        });
    }

    pub fn pause(&self) {
        run_on_main(&self.app, || with_engine_mut(|e| e.pause()));
    }
    pub fn resume(&self) {
        run_on_main(&self.app, || with_engine_mut(|e| e.resume()));
    }
    pub fn seek(&self, position_ms: u64) {
        run_on_main(&self.app, move || with_engine_mut(|e| e.seek(position_ms)));
    }
    pub fn next(&self) {
        run_on_main(&self.app, || with_engine_mut(|e| e.advance(1)));
    }
    pub fn previous(&self) {
        run_on_main(&self.app, || with_engine_mut(|e| e.previous()));
    }
    pub fn stop(&self) {
        run_on_main(&self.app, || with_engine_mut(|e| e.stop()));
    }
    pub fn set_volume(&self, volume: f64) {
        run_on_main(&self.app, move || with_engine_mut(|e| e.set_volume(volume as f32)));
    }
    pub fn set_rate(&self, rate: f64) {
        run_on_main(&self.app, move || with_engine_mut(|e| e.set_rate(rate as f32)));
    }
}

/// Run `f` on the main thread (where all AVPlayer / objc-UI mutation must happen). Best-effort.
fn run_on_main(app: &AppHandle, f: impl FnOnce() + Send + 'static) {
    if let Err(err) = app.run_on_main_thread(f) {
        log::warn!("native preview: run_on_main_thread failed: {err}");
    }
}

/// Access the live engine on the main thread, if it exists. `try_borrow_mut` guards the (practically
/// impossible, since the main thread is single-threaded) re-entrant borrow if a callback fires
/// mid-operation.
pub(super) fn with_engine_mut(f: impl FnOnce(&mut PlaybackEngineInner)) {
    ENGINE.with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut() {
            if let Some(engine) = slot.as_mut() {
                f(engine);
            }
        }
    });
}

// =============================================================================
// Events (backend → frontend). camelCase to match the TS payload types.
// =============================================================================

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StatePayload {
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
}

pub(super) fn emit_state(app: &AppHandle, payload: StatePayload) {
    let _ = app.emit("native-preview-state", payload);
}

pub(super) fn emit_track_changed(app: &AppHandle, index: usize) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        index: usize,
    }
    let _ = app.emit("native-preview-track-changed", Payload { index });
}

pub(super) fn emit_ended(app: &AppHandle) {
    let _ = app.emit("native-preview-ended", ());
}

pub(super) fn emit_error(app: &AppHandle, message: String) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        message: String,
    }
    let _ = app.emit("native-preview-error", Payload { message });
}
