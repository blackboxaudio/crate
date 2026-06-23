//! The AVPlayer-backed playback engine (main-thread only). Owns the player, the current playlist +
//! index, the periodic time observer, and the remote-command / NotificationCenter observer tokens.
//!
//! objc2 reconciliation surface: method names + the `Cargo.toml` framework `features` are validated on
//! device with `cargo check --target aarch64-apple-ios` (de-risking step #1).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{msg_send, MainThreadMarker};
use objc2_av_foundation::{
    AVAudioTimePitchAlgorithmVarispeed, AVPlayer, AVPlayerItem, AVPlayerItemStatus, AVURLAsset,
    AVURLAssetOverrideMIMETypeKey,
};
use objc2_core_media::CMTime;
use objc2_foundation::{NSDictionary, NSError, NSMutableDictionary, NSString, NSURL};
use tauri::AppHandle;

use super::engine::{self, NativeTrackEntry, StatePayload};
use super::{now_playing, observers, remote_command};

// Mirror shared/stores/player.ts: "previous" restarts the current track if past this window (or on
// the first track), else jumps to the previous track.
const PREVIOUS_RESTART_THRESHOLD_MS: u64 = 3000;

// CMTime timescale for second↔CMTime conversions (600 is the conventional value: divisible by common
// frame rates and fine enough for audio scrubbing).
const TIMESCALE: i32 = 600;

pub struct PlaybackEngineInner {
    app: AppHandle,
    player: Retained<AVPlayer>,
    // Main-thread token for AVFoundation constructors that require it. Safe to hold: the engine only
    // ever lives in (and is touched on) the main thread via the thread-local in `engine`.
    mtm: MainThreadMarker,
    entries: Vec<NativeTrackEntry>,
    index: usize,
    // Bumped on every `play_index`. The load watchdog captures the value at spawn time and bails if
    // it no longer matches — so a watchdog for a superseded track can't report a stale failure.
    epoch: u64,
    rate: f32,
    playing: bool,
    // True from a programmatic `seek` until its completion handler reports the seek landed. AVPlayer's
    // `currentTime` keeps returning the pre-seek position until then, so `tick` is suppressed while this
    // is set — otherwise the periodic position emit flashes the playhead back to the old spot.
    seeking: bool,
    time_observer: Option<Retained<AnyObject>>,
    // Held only to keep the command-handler / notification blocks alive for the engine's lifetime.
    _command_targets: Vec<Retained<AnyObject>>,
    _observers: Vec<Retained<AnyObject>>,
}

impl PlaybackEngineInner {
    /// Construct the engine on the main thread: create the AVPlayer, wire the lock-screen remote
    /// commands + audio-session observers, and install the periodic position observer.
    pub fn new(app: AppHandle) -> Self {
        // The engine is only ever constructed inside a `run_on_main_thread` closure, so we are on the
        // main thread here.
        let mtm = MainThreadMarker::new().expect("native preview engine must be created on main thread");
        // SAFETY: AVPlayer designated initializer; on the main thread (mtm proves it).
        let player = unsafe { AVPlayer::new(mtm) };
        let command_targets = remote_command::configure(&app);
        let observer_tokens = observers::register(&app);

        let mut inner = Self {
            app,
            player,
            mtm,
            entries: Vec::new(),
            index: 0,
            epoch: 0,
            rate: 1.0,
            playing: false,
            seeking: false,
            time_observer: None,
            _command_targets: command_targets,
            _observers: observer_tokens,
        };
        inner.install_time_observer();
        inner
    }

    fn install_time_observer(&mut self) {
        // SAFETY: CMTime constructor (core-media FFI).
        let interval = unsafe { CMTime::with_seconds(0.5, TIMESCALE) };
        let block = RcBlock::new(move |_time: CMTime| {
            engine::with_engine_mut(|e| e.tick());
        });
        // SAFETY: standard AVPlayer periodic-time-observer FFI. `None` queue ⇒ main queue, so the block
        // runs on the main thread (safe to touch the player + emit). The returned token is retained for
        // the engine's lifetime and removed in `Drop`.
        let token = unsafe {
            self.player
                .addPeriodicTimeObserverForInterval_queue_usingBlock(interval, None, &block)
        };
        self.time_observer = Some(token);
    }

    /// Replace the playlist and start playing from `start_index`.
    pub fn load(&mut self, entries: Vec<NativeTrackEntry>, start_index: usize) {
        if entries.is_empty() {
            return;
        }
        self.entries = entries;
        let i = start_index.min(self.entries.len() - 1);
        self.play_index(i);
    }

    /// Replace the UPCOMING tail (everything after the current index) without touching the
    /// currently-playing `AVPlayerItem`, `self.index`, or the lock-screen Now Playing card. The current
    /// track keeps playing and native auto-advance (`on_item_ended` → `advance(1)`, which reads
    /// `self.entries` live) rolls straight into the new tail. The already-played front (`entries[0..=index]`)
    /// is kept so the engine's track-changed indices stay valid and the lock-screen Previous can still step
    /// back into it. This is what lets the JS sliding window apply queue mutations / refill seamlessly,
    /// including while the screen is locked. No-op until something is loaded (the next `load` sets the window).
    pub fn set_upcoming(&mut self, upcoming: Vec<NativeTrackEntry>) {
        if self.entries.is_empty() {
            return;
        }
        let keep = (self.index + 1).min(self.entries.len());
        self.entries.truncate(keep);
        self.entries.extend(upcoming);
    }

    fn play_index(&mut self, i: usize) {
        let Some(entry) = self.entries.get(i).cloned() else {
            return;
        };
        self.index = i;
        self.epoch = self.epoch.wrapping_add(1);
        // New item ⇒ any in-flight seek on the previous one is moot; clear the guard so its late
        // completion handler (or a stale flag) can't suppress tracking on the fresh track.
        self.seeking = false;
        engine::emit_debug(
            &self.app,
            format!("load track {i} (mime={:?}): {}", entry.mime_type, entry.url),
        );
        // SAFETY: build an AVPlayerItem from the (already-resolved) proxy URL and make it current.
        unsafe {
            let url_str = NSString::from_str(&entry.url);
            let Some(url) = NSURL::URLWithString(&url_str) else {
                engine::emit_error(&self.app, format!("invalid stream url: {}", entry.url));
                return;
            };
            // Force the container type when the source provides one. The proxy URL is extensionless
            // (`…/{release}/{position}`); for YouTube/Discogs (`audio/mp4`) AVFoundation can't infer
            // the format from the URL and silently fails to load, so we hand it the MIME type via
            // `AVURLAssetOverrideMIMETypeKey`, which makes it ignore the URL/extension entirely.
            let item = if let Some(mime) = entry.mime_type.as_deref() {
                let opts: Retained<NSMutableDictionary<NSString, AnyObject>> =
                    NSMutableDictionary::new();
                let mime_val = NSString::from_str(mime);
                let _: () =
                    msg_send![&*opts, setObject: &*mime_val, forKey: AVURLAssetOverrideMIMETypeKey];
                let opts_ref: &NSDictionary<NSString, AnyObject> = &opts;
                let asset = AVURLAsset::URLAssetWithURL_options(&url, Some(opts_ref));
                AVPlayerItem::playerItemWithAsset(&asset, self.mtm)
            } else {
                AVPlayerItem::playerItemWithURL(&url, self.mtm)
            };
            // Vinyl-style tempo: pitch tracks tempo (no "master tempo" / key-lock), matching the HTML5
            // preview player (`preservesPitch = false`) and the desktop engine. AVPlayer otherwise
            // defaults to a pitch-preserving algorithm (`AVAudioTimePitchAlgorithmTimeDomain` on
            // iOS 15+) that holds pitch constant as the rate changes. We MUST use the framework's
            // exported constant here: its runtime value is the short string "Varispeed", NOT the symbol
            // name, so a hand-built `NSString::from_str("AVAudioTimePitchAlgorithmVarispeed")` is an
            // unrecognized value that AVFoundation silently ignores — leaving the pitch-preserving
            // default in place. The `AVAudioProcessingSettings` Cargo feature (enabled in Cargo.toml)
            // exposes the static; it is `Option` only for weak-link safety and is non-null on iOS 7+.
            if let Some(varispeed) = AVAudioTimePitchAlgorithmVarispeed {
                item.setAudioTimePitchAlgorithm(varispeed);
            }
            self.player.replaceCurrentItemWithPlayerItem(Some(&item));
            self.player.play();
            if (self.rate - 1.0).abs() > f32::EPSILON {
                self.player.setRate(self.rate);
            }
        }
        self.playing = true;
        now_playing::update(&self.app, &entry, 0.0, self.rate);
        engine::emit_track_changed(&self.app, self.index);
        self.emit_current_state();
        // AVPlayer load failures are otherwise silent — the periodic time observer doesn't tick while
        // an item is stuck loading, so a stream AVFoundation can't play would sit in a fake "playing"
        // state forever. Watch this item's status until it resolves and surface any failure.
        spawn_load_watchdog(self.app.clone(), self.epoch);
    }

    /// Poll the current item's load status (driven by [`spawn_load_watchdog`], off the player's
    /// timeline). Returns `true` once the outcome is decided so the watchdog can stop: the track was
    /// superseded, there's no current item, it became ready, or it FAILED — in which case the
    /// AVFoundation error is surfaced via [`Self::fail`].
    fn poll_load_status(&mut self, epoch: u64) -> bool {
        if self.epoch != epoch {
            return true; // a newer track replaced this one; its own watchdog owns it
        }
        // SAFETY: read the current item and its status/error (AVPlayerItem FFI; on the main thread).
        let Some(item) = (unsafe { self.player.currentItem() }) else {
            return true;
        };
        let status = unsafe { item.status() };
        if status == AVPlayerItemStatus::Failed {
            let message = unsafe { item.error() }
                .map(|e| nserror_message(&e))
                .unwrap_or_else(|| "AVPlayer reported a failed item with no error".to_string());
            engine::emit_debug(&self.app, format!("status=Failed on track {}", self.index));
            self.fail(message);
            return true;
        }
        if status == AVPlayerItemStatus::ReadyToPlay {
            engine::emit_debug(&self.app, format!("status=ReadyToPlay on track {}", self.index));
            return true;
        }
        false
    }

    /// Surface a playback failure: log it (→ `yarn dev:ios` terminal via env_logger/stderr), tell the
    /// frontend (→ error toast), and drop out of the fake "playing" state. Shared by the load
    /// watchdog and the `FailedToPlayToEndTime` notification.
    pub(super) fn fail(&mut self, message: String) {
        log::error!(
            "native preview: playback failed on track {}: {message}",
            self.index
        );
        self.playing = false;
        engine::emit_error(&self.app, message);
        engine::emit_state(
            &self.app,
            StatePayload {
                is_playing: false,
                position_ms: 0,
                duration_ms: 0,
            },
        );
    }

    pub fn pause(&mut self) {
        // SAFETY: AVPlayer.pause is main-thread safe.
        unsafe { self.player.pause() };
        self.playing = false;
        now_playing::set_playback(self.position_secs(), 0.0);
        self.emit_current_state();
    }

    pub fn resume(&mut self) {
        // SAFETY: AVPlayer.play / setRate are main-thread safe.
        unsafe {
            self.player.play();
            if (self.rate - 1.0).abs() > f32::EPSILON {
                self.player.setRate(self.rate);
            }
        }
        self.playing = true;
        now_playing::set_playback(self.position_secs(), self.rate);
        self.emit_current_state();
    }

    /// play/pause toggle — used by the lock-screen togglePlayPause command.
    pub fn toggle(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.resume();
        }
    }

    pub fn seek(&mut self, position_ms: u64) {
        let secs = position_ms as f64 / 1000.0;
        self.seeking = true;
        let app = self.app.clone();
        // Fires once the async seek lands. AVFoundation may invoke it off the main thread (and possibly
        // synchronously if no item is attached), so it hops back to the main thread before touching the
        // main-thread-only engine. `finished == NO` means a newer seek superseded this one — leave the
        // guard set so tracking stays suppressed until that newer seek's handler lands.
        let handler = RcBlock::new(move |finished: Bool| {
            if !finished.as_bool() {
                return;
            }
            let _ = app.run_on_main_thread(|| {
                engine::with_engine_mut(|e| {
                    e.seeking = false;
                    e.emit_current_state();
                });
            });
        });
        // SAFETY: build a CMTime and seek within the current item (core-media + AVPlayer FFI).
        unsafe {
            let t = CMTime::with_seconds(secs, TIMESCALE);
            self.player.seekToTime_completionHandler(t, &handler);
        }
        now_playing::set_playback(secs, if self.playing { self.rate } else { 0.0 });
        // Snap the UI straight to the requested target. Reading `currentTime` back here would report the
        // PRE-seek position (the seek is async), which is the flash-back bug; the completion handler
        // re-emits the real position once the seek settles.
        engine::emit_state(
            &self.app,
            StatePayload {
                is_playing: self.playing,
                position_ms,
                duration_ms: (self.duration_secs() * 1000.0) as u64,
            },
        );
    }

    /// Move by `delta` tracks. Before the first track restarts it; past the last track stops and emits
    /// `native-preview-ended`.
    pub fn advance(&mut self, delta: i64) {
        let next = self.index as i64 + delta;
        if next < 0 {
            self.seek(0);
            return;
        }
        if next as usize >= self.entries.len() {
            engine::emit_debug(
                &self.app,
                format!("advance past last track ({}/{}) → stop + ended", next, self.entries.len()),
            );
            self.stop();
            engine::emit_ended(&self.app);
            return;
        }
        self.play_index(next as usize);
    }

    /// "Previous" with the shared 3s restart-vs-previous rule.
    pub fn previous(&mut self) {
        if (self.position_secs() * 1000.0) as u64 > PREVIOUS_RESTART_THRESHOLD_MS || self.index == 0 {
            self.seek(0);
        } else {
            self.advance(-1);
        }
    }

    /// AVPlayerItemDidPlayToEndTime → advance to the next track (or end).
    pub fn on_item_ended(&mut self) {
        engine::emit_debug(
            &self.app,
            format!("AVPlayerItemDidPlayToEndTime fired on track {}", self.index),
        );
        self.advance(1);
    }

    pub fn stop(&mut self) {
        // SAFETY: pause + detach the current item.
        unsafe {
            self.player.pause();
            self.player.replaceCurrentItemWithPlayerItem(None);
        }
        self.playing = false;
        self.seeking = false;
        now_playing::clear();
        engine::emit_state(
            &self.app,
            StatePayload {
                is_playing: false,
                position_ms: 0,
                duration_ms: 0,
            },
        );
    }

    pub fn set_volume(&mut self, volume: f32) {
        // SAFETY: AVPlayer.volume setter.
        unsafe { self.player.setVolume(volume.clamp(0.0, 1.0)) };
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.9, 1.1);
        if self.playing {
            // SAFETY: setRate also resumes playback at the given rate.
            unsafe { self.player.setRate(self.rate) };
            now_playing::set_playback(self.position_secs(), self.rate);
        }
    }

    /// Periodic tick (≈2×/sec): push position to the frontend + keep the lock-screen elapsed/rate live.
    fn tick(&mut self) {
        // A seek is in flight: AVPlayer still reports the pre-seek `currentTime`, so skip this tick to
        // avoid flashing the playhead back. The seek's completion handler re-emits once it settles.
        if self.seeking {
            return;
        }
        self.emit_current_state();
        now_playing::set_playback(self.position_secs(), if self.playing { self.rate } else { 0.0 });
    }

    fn position_secs(&self) -> f64 {
        // SAFETY: currentTime + seconds() are AVPlayer / core-media FFI; valid once an item is loaded.
        let t = unsafe { self.player.currentTime().seconds() };
        if t.is_finite() && t >= 0.0 {
            t
        } else {
            0.0
        }
    }

    fn duration_secs(&self) -> f64 {
        // SAFETY: read the current item's duration; it can be NaN/indefinite before the item is ready.
        unsafe {
            if let Some(item) = self.player.currentItem() {
                let d = item.duration().seconds();
                if d.is_finite() && d > 0.0 {
                    return d;
                }
            }
        }
        // Fall back to the metadata duration for the current entry until the item reports its own.
        self.entries
            .get(self.index)
            .map(|e| e.duration_ms as f64 / 1000.0)
            .unwrap_or(0.0)
    }

    fn emit_current_state(&self) {
        engine::emit_state(
            &self.app,
            StatePayload {
                is_playing: self.playing,
                position_ms: (self.position_secs() * 1000.0) as u64,
                duration_ms: (self.duration_secs() * 1000.0) as u64,
            },
        );
    }
}

impl Drop for PlaybackEngineInner {
    fn drop(&mut self) {
        if let Some(token) = self.time_observer.take() {
            // SAFETY: the token came from addPeriodicTimeObserver on this player; remove before drop.
            unsafe { self.player.removeTimeObserver(&token) };
        }
    }
}

/// Format an `NSError` for logging + the frontend: localized description plus the domain/code that
/// pin down the cause (e.g. `[NSURLErrorDomain -1100]` for a 404/unreachable stream, or an
/// `AVFoundationErrorDomain` code for an undecodable container).
fn nserror_message(err: &NSError) -> String {
    let desc = err.localizedDescription();
    let domain = err.domain();
    let code = err.code();
    format!("{desc} [{domain} {code}]")
}

/// Watch a freshly-loaded item until its load resolves. The periodic time observer only fires while
/// the player's timebase advances, so an item that never becomes playable would never be noticed;
/// this polls `AVPlayerItem.status` on the main thread a few times a second instead. It self-stops
/// once the load is decided (ready / failed / superseded) and after a bounded window regardless.
fn spawn_load_watchdog(app: AppHandle, epoch: u64) {
    let resolved = Arc::new(AtomicBool::new(false));
    tauri::async_runtime::spawn(async move {
        // ~8s of coverage (20 × 400ms): enough for the proxy's first full download + the AVPlayer
        // load to settle, short enough not to linger. Stops early the moment the load resolves.
        for _ in 0..20 {
            if resolved.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
            let resolved_cb = resolved.clone();
            let _ = app.run_on_main_thread(move || {
                engine::with_engine_mut(|e| {
                    if e.poll_load_status(epoch) {
                        resolved_cb.store(true, Ordering::Relaxed);
                    }
                });
            });
        }
        if !resolved.load(Ordering::Relaxed) {
            engine::emit_debug(
                &app,
                "watchdog timed out: item never became ready or failed (still loading/Unknown after ~8s)"
                    .to_string(),
            );
        }
    });
}
