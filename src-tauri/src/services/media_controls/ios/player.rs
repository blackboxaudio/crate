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
    AVAudioTimePitchAlgorithmVarispeed, AVPlayer, AVPlayerItem, AVPlayerItemStatus,
    AVPlayerTimeControlStatus, AVURLAsset, AVURLAssetOverrideMIMETypeKey,
};
use objc2_core_media::CMTime;
use objc2_foundation::{NSDictionary, NSError, NSMutableDictionary, NSString, NSURL};
use tauri::{AppHandle, Manager};

use super::engine::{self, NativeTrackEntry, StatePayload};
use super::{now_playing, observers, remote_command};

// Mirror shared/stores/player.ts: "previous" restarts the current track if past this window (or on
// the first track), else jumps to the previous track.
const PREVIOUS_RESTART_THRESHOLD_MS: u64 = 3000;

// Repeat-track flag (set from JS via `native_preview_set_repeat_mode`): when on, an item that plays
// to its end is rewound and replayed instead of advancing — gapless, and it keeps working while the
// WebView's JS is suspended on lock. Lives OUTSIDE the engine struct so the mode pushed at bridge
// start (before the engine is lazily constructed on first play) isn't lost. Only ever touched on
// the main thread; Relaxed is plenty.
static REPEAT_CURRENT: AtomicBool = AtomicBool::new(false);

pub(super) fn set_repeat_current(enabled: bool) {
    REPEAT_CURRENT.store(enabled, Ordering::Relaxed);
}

// CMTime timescale for second↔CMTime conversions (600 is the conventional value: divisible by common
// frame rates and fine enough for audio scrubbing).
const TIMESCALE: i32 = 600;

// Load/stall watchdog cadence. The periodic time observer can't cover a loading or stalled item (it
// only fires while the timebase advances, which is exactly what isn't happening), so this poll is the
// sole driver of `is_buffering` updates during a load.
//
// It ramps: a pre-fed window item is usually audible within a few tens of ms, and at a flat 400ms
// cadence every gapless advance would flash the frontend's spinner for the remainder of the first
// tick. The fast phase resolves those before anyone sees them; the slow phase then covers the long
// cold-start tail cheaply. The total window is generous because a cold track waits on the proxy's
// full download before AVPlayer sees a byte — past it we declare failure rather than leaving the
// user on a spinner forever.
const LOAD_WATCHDOG_FAST_POLL_MS: u64 = 100;
const LOAD_WATCHDOG_FAST_TICKS: u32 = 8; // 0.8s
const LOAD_WATCHDOG_POLL_MS: u64 = 400;
const LOAD_WATCHDOG_SLOW_TICKS: u32 = 36; // 14.4s → ~15.2s of coverage total

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
    // Epoch whose failure has already been surfaced. `fail` is reachable from BOTH the load watchdog
    // and the `FailedToPlayToEndTime` notification for the same item; this latch collapses them into
    // one emitted error (= one user-facing toast). Cleared on every new item.
    failed_epoch: Option<u64>,
    rate: f32,
    playing: bool,
    // Whether playback was active when an audio-session interruption began. `pause()` clobbers
    // `playing`, so without this latch the `.ended` handler can't tell "resume what the phone call
    // interrupted" from "start playing something the user had deliberately paused".
    playing_before_interruption: bool,
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
        let mtm =
            MainThreadMarker::new().expect("native preview engine must be created on main thread");
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
            failed_epoch: None,
            rate: 1.0,
            playing: false,
            playing_before_interruption: false,
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

    /// Replace the playlist and start playing from `start_index`, beginning `start_position_ms` into
    /// that track (0 = from the start; non-zero only when restoring the last session on app relaunch).
    pub fn load(
        &mut self,
        entries: Vec<NativeTrackEntry>,
        start_index: usize,
        start_position_ms: u64,
    ) {
        if entries.is_empty() {
            return;
        }
        self.entries = entries;
        let i = start_index.min(self.entries.len() - 1);
        self.play_index_at(i, start_position_ms);
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
        self.play_index_at(i, 0);
    }

    /// Like [`Self::play_index`] but begins playback `start_position_ms` into the track (0 = from the
    /// start). A non-zero offset is used when restoring the last session on relaunch: the item is sought
    /// to the saved position BEFORE it starts rendering, so the first audio (and the emitted/lock-screen
    /// position) is at the offset rather than blipping from the start. AVPlayer queues a seek issued
    /// before the item is ready and applies it once ready, so the seek-then-play order avoids the flash.
    fn play_index_at(&mut self, i: usize, start_position_ms: u64) {
        let Some(entry) = self.entries.get(i).cloned() else {
            return;
        };
        self.index = i;
        self.epoch = self.epoch.wrapping_add(1);
        self.failed_epoch = None;
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
                engine::emit_error(
                    &self.app,
                    format!("invalid stream url: {}", entry.url),
                    true,
                );
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
        }
        self.playing = true;
        let start_secs = start_position_ms as f64 / 1000.0;
        now_playing::update(&self.app, &entry, start_secs, self.rate);
        remote_command::set_like_state(entry.is_liked);
        engine::emit_track_changed(&self.app, self.index);
        // Position the item at the restore offset BEFORE starting playback. `self.seek` queues the seek
        // (AVPlayer applies it once the item is ready), sets the `seeking` guard so the periodic observer
        // doesn't flash the playhead to 0 in the meantime, and emits the target position straight away.
        if start_position_ms > 0 {
            self.seek(start_position_ms);
        }
        // SAFETY: AVPlayer.play / setRate are main-thread safe. Started AFTER the seek above so playback
        // begins at the offset rather than rendering from the start first.
        unsafe {
            self.player.play();
            if (self.rate - 1.0).abs() > f32::EPSILON {
                self.player.setRate(self.rate);
            }
        }
        // Fresh-from-start track: emit the initial (0) state now. For a restore, `self.seek` above already
        // emitted the offset position (and suppressed the periodic observer until the seek lands).
        //
        // `is_buffering` is asserted rather than read: a just-replaced item cannot be rendering audio,
        // but `timeControlStatus` may not have caught up to the swap yet and can still report the
        // OUTGOING item's `Playing` — which would blink the frontend's loading spinner off and back on
        // one poll later. The watchdog below owns the flag from here.
        if start_position_ms == 0 {
            engine::emit_state(
                &self.app,
                StatePayload {
                    is_playing: self.playing,
                    position_ms: 0,
                    duration_ms: (self.duration_secs() * 1000.0) as u64,
                    is_buffering: true,
                },
            );
        }
        // AVPlayer load failures are otherwise silent — the periodic time observer doesn't tick while
        // an item is stuck loading, so a stream AVFoundation can't play would sit in a fake "playing"
        // state forever. Watch this item until audio is genuinely rolling, and surface any failure.
        spawn_load_watchdog(self.app.clone(), self.epoch);
    }

    /// Poll the current item toward an audible outcome (driven by [`spawn_load_watchdog`], off the
    /// player's timeline). Returns `true` once the outcome is decided so the watchdog can stop: the
    /// track was superseded, there's no current item, it FAILED (surfaced via [`Self::fail`]), or
    /// audio is genuinely rolling.
    ///
    /// While the outcome is still pending this emits state on every poll, which is what keeps the
    /// frontend's loading spinner up: `ReadyToPlay` is deliberately NOT a stop condition, because an
    /// item can be ready and still silent while its buffer fills. Only `timeControlStatus == Playing`
    /// (or the user pausing out from under us) counts as decided.
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
        // Decided once audio is rolling — or once the user pauses out from under the load, which
        // `is_buffering` also reports as false, so the emit below hands the spinner over to the paused
        // transport rather than stranding it.
        let decided = !self.is_buffering();
        // A restore-offset seek is still in flight: `currentTime` reads pre-seek, so emitting here
        // would flash the playhead to 0 (same reason `tick` bails). Keep polling, just stay quiet.
        if !self.seeking {
            self.emit_current_state();
        }
        if decided {
            engine::emit_debug(
                &self.app,
                format!("load resolved on track {} (playing={})", self.index, self.playing),
            );
        }
        decided
    }

    /// The load/stall watchdog exhausted its window without the item ever becoming audible. Report it
    /// as a NON-retryable failure: unlike a dead upstream URL, re-resolving would just walk the same
    /// slow path again, so the frontend goes straight to idle + toast instead of doubling the wait.
    pub(super) fn fail_load_timeout(&mut self, epoch: u64) {
        if self.epoch != epoch {
            return; // a newer track replaced this one; its own watchdog owns it
        }
        let window_ms = LOAD_WATCHDOG_FAST_TICKS as u64 * LOAD_WATCHDOG_FAST_POLL_MS
            + LOAD_WATCHDOG_SLOW_TICKS as u64 * LOAD_WATCHDOG_POLL_MS;
        self.fail_with(
            format!("stream never became playable within {}s", window_ms / 1000),
            false,
        );
    }

    /// Surface a playback failure: log it (→ `yarn dev:ios` terminal via env_logger/stderr), tell the
    /// frontend (→ error toast), and drop out of the fake "playing" state. Shared by the load
    /// watchdog and the `FailedToPlayToEndTime` notification; duplicate reports for the same item are
    /// collapsed via `failed_epoch` so one failure never emits two errors.
    ///
    /// These are the retryable failures — an AVFoundation load error usually means a dead or expired
    /// upstream URL, which the frontend's one-shot re-resolve genuinely fixes.
    pub(super) fn fail(&mut self, message: String) {
        self.fail_with(message, true);
    }

    fn fail_with(&mut self, message: String, retryable: bool) {
        if self.failed_epoch == Some(self.epoch) {
            return;
        }
        self.failed_epoch = Some(self.epoch);
        log::error!(
            "native preview: playback failed on track {}: {message}",
            self.index
        );
        self.playing = false;
        engine::emit_error(&self.app, message, retryable);
        engine::emit_state(
            &self.app,
            StatePayload {
                is_playing: false,
                position_ms: 0,
                duration_ms: 0,
                is_buffering: false,
            },
        );
    }

    /// `AVPlayerItemPlaybackStalled` → the buffer ran dry mid-track. Emit right away so the spinner
    /// comes back, then re-arm the watchdog to drive the recovery (it clears the spinner when audio
    /// resumes, and fails out if it never does — otherwise a permanent stall would spin forever).
    pub(super) fn on_playback_stalled(&mut self) {
        engine::emit_debug(
            &self.app,
            format!("playback stalled on track {}", self.index),
        );
        self.emit_current_state();
        spawn_load_watchdog(self.app.clone(), self.epoch);
    }

    /// Lock-screen Like: toggle the current track's liked state natively — this must work while
    /// the WebView's JS is suspended, so the DB write happens here, not via a JS round-trip. The
    /// write runs off the main thread; the glyph update + frontend notification hop back onto it
    /// (same thread dance as the Now Playing artwork download).
    pub(super) fn like_pressed(&mut self) {
        let Some(entry) = self.entries.get(self.index) else {
            return;
        };
        let Some(track_id) = entry.track_id.clone() else {
            engine::emit_debug(
                &self.app,
                "like pressed but current entry has no track_id".into(),
            );
            return;
        };
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let toggled = {
                let discovery = app.state::<crate::services::DiscoveryService>();
                discovery.toggle_track_liked(&track_id)
            };
            match toggled {
                Ok(liked) => {
                    let app2 = app.clone();
                    let tid = track_id.clone();
                    let _ = app.run_on_main_thread(move || {
                        engine::with_engine_mut(|e| e.apply_liked(&tid, liked));
                        engine::emit_like_changed(&app2, &tid, liked);
                    });
                }
                Err(e) => log::warn!("native preview: like toggle failed: {e}"),
            }
        });
    }

    /// Set liked on every entry with this track id (the window can hold repeats); refresh the
    /// lock-screen glyph when it's the current one.
    pub(super) fn apply_liked(&mut self, track_id: &str, liked: bool) {
        let mut is_current = false;
        for (i, entry) in self.entries.iter_mut().enumerate() {
            if entry.track_id.as_deref() == Some(track_id) {
                entry.is_liked = liked;
                if i == self.index {
                    is_current = true;
                }
            }
        }
        if is_current {
            remote_command::set_like_state(liked);
        }
    }

    pub fn pause(&mut self) {
        // SAFETY: AVPlayer.pause is main-thread safe.
        unsafe { self.player.pause() };
        self.playing = false;
        now_playing::set_playback(self.position_secs(), 0.0);
        self.emit_current_state();
    }

    pub fn resume(&mut self) {
        // Nothing loaded — refuse to "resume". The lock-screen play/toggle commands stay enabled
        // for the process lifetime (the engine thread-local is never cleared), and Bluetooth/AVRCP
        // devices routinely send PLAY on connect, so an unguarded resume here would start audio the
        // user never asked for.
        if self.entries.is_empty() {
            return;
        }

        // SAFETY: AVPlayer.play / setRate are main-thread safe.
        unsafe {
            self.player.play();
            if (self.rate - 1.0).abs() > f32::EPSILON {
                self.player.setRate(self.rate);
            }
        }
        self.playing = true;
        // A resume is a fresh attempt at making this item audible, so clear the one-error-per-item
        // latch: without this, a track that already reported a failure (e.g. a load timeout the user
        // is retrying by hitting play again) could never report a second one, and its watchdog would
        // expire silently — leaving the spinner up forever.
        self.failed_epoch = None;
        now_playing::set_playback(self.position_secs(), self.rate);
        self.emit_current_state();
        // A resume can stall just like a fresh load (buffer drained while paused, network changed),
        // so re-arm the watchdog to drive the spinner and time the resume out if it never starts.
        spawn_load_watchdog(self.app.clone(), self.epoch);
    }

    /// Pause because an audio-session interruption began (phone call, Siri, another app taking the
    /// session), remembering whether we were actually playing so `resume_after_interruption` can
    /// restore exactly that.
    pub fn pause_for_interruption(&mut self) {
        self.playing_before_interruption = self.playing;
        self.pause();
    }

    /// Resume after an interruption ended — only if we were playing when it began. iOS setting
    /// `ShouldResume` means "you may resume", not "start playing".
    pub fn resume_after_interruption(&mut self) {
        if !self.playing_before_interruption {
            return;
        }
        self.playing_before_interruption = false;
        self.resume();
    }

    /// Pause because the output route we were playing on disappeared (headphones unplugged,
    /// Bluetooth device powered off). Clears the interruption latch so an unrelated interruption
    /// ending later can't resurrect playback onto the built-in speaker.
    pub fn pause_for_route_loss(&mut self) {
        self.playing_before_interruption = false;
        self.pause();
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
                is_buffering: self.is_buffering(),
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
                format!(
                    "advance past last track ({}/{}) → stop + ended",
                    next,
                    self.entries.len()
                ),
            );
            self.stop();
            engine::emit_ended(&self.app);
            return;
        }
        self.play_index(next as usize);
    }

    /// "Previous" with the shared 3s restart-vs-previous rule.
    pub fn previous(&mut self) {
        if (self.position_secs() * 1000.0) as u64 > PREVIOUS_RESTART_THRESHOLD_MS || self.index == 0
        {
            self.seek(0);
        } else {
            self.advance(-1);
        }
    }

    /// AVPlayerItemDidPlayToEndTime → loop the current item (repeat-track) or advance to the next
    /// track (or end).
    pub fn on_item_ended(&mut self) {
        engine::emit_debug(
            &self.app,
            format!("AVPlayerItemDidPlayToEndTime fired on track {}", self.index),
        );
        if REPEAT_CURRENT.load(Ordering::Relaxed) {
            // Repeat-track: rewind the SAME item and keep playing — no item swap, no `ended`
            // emission, no JS round-trip, so the loop is gapless and survives lock. AVPlayer
            // pauses itself at the end of an item, so play must be re-asserted after the rewind
            // (`self.playing` is still true — the pause was the item's, not the user's).
            self.seek(0);
            // SAFETY: AVPlayer.play / setRate are main-thread safe.
            unsafe {
                self.player.play();
                if (self.rate - 1.0).abs() > f32::EPSILON {
                    self.player.setRate(self.rate);
                }
            }
            return;
        }
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
                is_buffering: false,
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
        now_playing::set_playback(
            self.position_secs(),
            if self.playing { self.rate } else { 0.0 },
        );
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

    /// Whether we intend to play but AVPlayer isn't actually rendering audio — the item is still
    /// loading, or playback stalled on an empty buffer. `self.playing` alone can't answer this: it is
    /// set optimistically the instant a load is requested, which is precisely why the frontend used to
    /// drop its loading spinner into a stretch of silence.
    ///
    /// A user-initiated pause reports `Paused`, not `WaitingToPlayAtSpecifiedRate`, and is additionally
    /// excluded by the `self.playing` check — so pausing mid-load clears the spinner rather than
    /// stranding it.
    fn is_buffering(&self) -> bool {
        if !self.playing {
            return false;
        }
        // SAFETY: AVPlayer.timeControlStatus (iOS 10+), read on the main thread.
        let status = unsafe { self.player.timeControlStatus() };
        status == AVPlayerTimeControlStatus::WaitingToPlayAtSpecifiedRate
    }

    fn emit_current_state(&self) {
        engine::emit_state(
            &self.app,
            StatePayload {
                is_playing: self.playing,
                position_ms: (self.position_secs() * 1000.0) as u64,
                duration_ms: (self.duration_secs() * 1000.0) as u64,
                is_buffering: self.is_buffering(),
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

/// Watch a freshly-loaded (or resumed, or stalled) item until audio is genuinely rolling. The
/// periodic time observer only fires while the player's timebase advances, so an item that hasn't
/// started would never be noticed; this polls on the main thread a few times a second instead.
///
/// It does double duty: each poll emits state, so this is what keeps the frontend's loading spinner
/// up for exactly as long as the track is silent, and it self-stops once the outcome is decided
/// (audible / paused / failed / superseded). If the bounded window expires with the item still
/// silent, it reports a non-retryable failure so the UI falls back to idle with an error rather than
/// spinning indefinitely.
fn spawn_load_watchdog(app: AppHandle, epoch: u64) {
    let resolved = Arc::new(AtomicBool::new(false));
    tauri::async_runtime::spawn(async move {
        let schedule = std::iter::repeat(LOAD_WATCHDOG_FAST_POLL_MS)
            .take(LOAD_WATCHDOG_FAST_TICKS as usize)
            .chain(std::iter::repeat(LOAD_WATCHDOG_POLL_MS).take(LOAD_WATCHDOG_SLOW_TICKS as usize));
        for delay_ms in schedule {
            if resolved.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
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
            let _ = app.run_on_main_thread(move || {
                engine::with_engine_mut(|e| e.fail_load_timeout(epoch));
            });
        }
    });
}
