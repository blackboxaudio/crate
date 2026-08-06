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
    /// Optional explicit MIME type for the stream. When set, the engine builds the `AVPlayerItem`
    /// via an `AVURLAsset` with `AVURLAssetOverrideMIMETypeKey` so AVFoundation doesn't have to infer
    /// the container from the extensionless proxy URL — needed for YouTube/Discogs (`audio/mp4`),
    /// where inference otherwise fails silently. `None` keeps the plain URL initializer (Bandcamp/
    /// SoundCloud are unambiguous `audio/mpeg`).
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Identity + liked state for the lock-screen Like (feedback) command: the engine toggles the
    /// DB natively while the WebView's JS is suspended, so it needs the track's row id in hand.
    /// Defaults keep older/partial payloads deserializable; a `None` id disables Like for that entry.
    #[serde(default)]
    pub track_id: Option<String>,
    #[serde(default)]
    pub is_liked: bool,
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

    /// Load a release's tracks (pre-resolved proxy URLs) and start playing from `start_index`, beginning
    /// `start_position_ms` into that track (0 = from the start; non-zero only when restoring the last
    /// session on app relaunch, so playback resumes where it left off without a blip from the start).
    /// `load_id` is stamped on every track-changed/ended event this playlist emits (see the command doc).
    pub fn play(
        &self,
        tracks: Vec<NativeTrackEntry>,
        start_index: usize,
        start_position_ms: u64,
        load_id: u64,
    ) {
        let app = self.app.clone();
        run_on_main(&self.app, move || {
            ENGINE.with(|cell| {
                let mut slot = cell.borrow_mut();
                let engine = slot.get_or_insert_with(|| PlaybackEngineInner::new(app.clone()));
                engine.load(tracks, start_index, start_position_ms, load_id);
            });
        });
    }

    /// Replace the upcoming tail in place (slide the window / apply a queue mutation) without disturbing
    /// the currently-playing item.
    pub fn set_upcoming(&self, tracks: Vec<NativeTrackEntry>) {
        run_on_main(&self.app, move || {
            with_engine_mut(|e| e.set_upcoming(tracks))
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
        run_on_main(&self.app, move || {
            with_engine_mut(|e| e.set_volume(volume as f32))
        });
    }
    pub fn set_rate(&self, rate: f64) {
        run_on_main(&self.app, move || {
            with_engine_mut(|e| e.set_rate(rate as f32))
        });
    }

    /// Reflect an in-app like toggle on the engine's entries + the lock-screen glyph. The reverse
    /// direction (a lock-screen press) toggles natively in `like_pressed` and notifies JS via
    /// `native-preview-like-changed`.
    pub fn set_liked(&self, track_id: String, liked: bool) {
        run_on_main(&self.app, move || {
            with_engine_mut(|e| e.apply_liked(&track_id, liked))
        });
    }

    /// Apply the app's repeat mode: `track` loops the ending item natively (gapless, keeps working
    /// while the screen is locked); the lock-screen repeat glyph shows One/All/Off (release and
    /// context both display "All" — MPRepeatType has no finer notion). Deliberately does NOT
    /// require the engine to exist: the flag and the glyph live outside it, so the mode pushed at
    /// bridge start (before the first play lazily constructs the engine) sticks.
    pub fn set_repeat_mode(&self, mode: String) {
        run_on_main(&self.app, move || {
            super::player::set_repeat_current(mode == "track");
            let display = match mode.as_str() {
                "track" => 1,               // MPRepeatTypeOne
                "release" | "context" => 2, // MPRepeatTypeAll
                _ => 0,                     // MPRepeatTypeOff
            };
            super::remote_command::set_repeat_display(display);
        });
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
    /// We *want* to play but AVPlayer isn't actually rendering audio yet (item still loading, or
    /// stalled mid-track). This is the frontend's only honest "not audible yet" signal — the
    /// `is_playing` flag is set optimistically the moment a load is requested, so on its own it
    /// would clear the loading spinner while the track is still silent. Mirrors the HTML5 path's
    /// `waiting` / `playing` events.
    pub is_buffering: bool,
}

pub(super) fn emit_state(app: &AppHandle, payload: StatePayload) {
    let _ = app.emit("native-preview-state", payload);
}

pub(super) fn emit_track_changed(app: &AppHandle, index: usize, load_id: u64) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        index: usize,
        load_id: u64,
    }
    let _ = app.emit("native-preview-track-changed", Payload { index, load_id });
}

pub(super) fn emit_ended(app: &AppHandle, load_id: u64) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        load_id: u64,
    }
    let _ = app.emit("native-preview-ended", Payload { load_id });
}

/// Surface a playback failure to the frontend. `retryable` says whether re-resolving the stream
/// could plausibly fix it: an AVFoundation load error usually means a dead/expired upstream URL
/// (worth one silent retry), whereas a load *timeout* means the same slow path would just be walked
/// again — so the frontend goes straight to idle + toast instead of doubling the wait.
pub(super) fn emit_error(app: &AppHandle, message: String, retryable: bool) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        message: String,
        retryable: bool,
    }
    let _ = app.emit("native-preview-error", Payload { message, retryable });
}

/// The lock-screen repeat command chose a new OS repeat state (`off`/`one`/`all`). The frontend
/// owns the app-level mode: it maps one→track / all→context, applies it, and echoes the resulting
/// display state back via `native_preview_set_repeat_mode`. Best-effort like the other events.
pub(super) fn emit_repeat_changed(app: &AppHandle, mode: &str) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        mode: String,
    }
    let _ = app.emit(
        "native-preview-repeat-changed",
        Payload {
            mode: mode.to_string(),
        },
    );
}

/// A lock-screen Like press toggled the DB natively; tell JS (best-effort — if it's suspended,
/// stores re-read from the DB on next launch anyway) so the in-memory stores catch up.
pub(super) fn emit_like_changed(app: &AppHandle, track_id: &str, is_liked: bool) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        track_id: String,
        is_liked: bool,
    }
    let _ = app.emit(
        "native-preview-like-changed",
        Payload {
            track_id: track_id.to_string(),
            is_liked,
        },
    );
}

/// Temporary diagnostic channel (#54 debugging): log to the Rust logger AND push to the frontend so
/// the message shows up in the webview console / a debug toast — the iOS device's `yarn dev:ios`
/// terminal doesn't surface env_logger output, so this is how we trace the engine on-device.
pub(super) fn emit_debug(app: &AppHandle, message: String) {
    log::info!("native preview: {message}");
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        message: String,
    }
    let _ = app.emit("native-preview-debug", Payload { message });
}
