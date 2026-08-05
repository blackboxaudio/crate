//! Output-device watcher: pauses playback when the device we were playing on disappears, and
//! keeps the settings device list fresh.
//!
//! Detection is primary/secondary:
//!
//! - **Primary (instant).** cpal already registers a native `kAudioDevicePropertyDeviceIsAlive`
//!   listener per stream on macOS, and maps WASAPI's `AUDCLNT_E_DEVICE_INVALIDATED` on Windows.
//!   Both surface as [`rodio::cpal::StreamError::DeviceNotAvailable`] through the error callback
//!   wired up in `create_output_stream`, which arrives here as [`DeviceEvent::StreamLost`].
//! - **Secondary (poll).** A slow tick re-enumerates devices to catch the system default moving
//!   while the old device is still alive, to refresh the settings list, and as the only detector
//!   on Linux — ALSA reports runtime failures as `BackendSpecific`, not `DeviceNotAvailable`.
//!   (Under PulseAudio/PipeWire the route usually moves transparently and nothing is reported at
//!   all; that is an acceptable outcome, not something to chase.)
//!
//! On loss the stream is rebuilt on the new default but left **paused** — matching iOS, which
//! pauses on `AVAudioSessionRouteChangeReasonOldDeviceUnavailable`. Playback must never follow
//! the user's headphones out onto the built-in speakers.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::{AudioService, PlaybackState};
use crate::models::AudioDevice;

/// Emitted when the output device backing playback disappeared.
pub const DEVICE_LOST_EVENT: &str = "audio-output-device-lost";
/// Emitted when the set of available output devices (or the system default) changed.
pub const DEVICES_CHANGED_EVENT: &str = "audio-devices-changed";

/// How often to re-enumerate output devices.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Window used to coalesce a burst of loss events. A Bluetooth drop can fire the alive-listener
/// and a default-device change back to back; waiting also lets the OS settle on the new default
/// before we ask for it, which it otherwise may not have done yet.
const LOSS_DEBOUNCE: Duration = Duration::from_millis(250);
/// Ignore a fresh loss within this window of the last rebuild, so a flapping Bluetooth link
/// can't drive a rebuild storm.
const REBUILD_COOLDOWN: Duration = Duration::from_secs(1);

/// Raised by the cpal stream error callback, which runs on a CoreAudio HAL notification thread
/// (macOS) or the WASAPI run loop (Windows). Deliberately `Copy` and allocation-free: that
/// callback must do nothing but send.
#[derive(Debug, Clone, Copy)]
pub(super) enum DeviceEvent {
    /// The device backing stream generation `epoch` is gone.
    StreamLost { epoch: u64 },
    /// A non-fatal backend error on generation `epoch`; logged only.
    StreamError { epoch: u64 },
}

/// Payload for [`DEVICE_LOST_EVENT`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLostPayload {
    /// Device the stream was playing on when it died. `None` if the name couldn't be read.
    pub lost_device: Option<String>,
    /// Device the stream was rebuilt on. `None` when no output device exists at all.
    pub new_device: Option<String>,
    /// Whether audible playback was actually interrupted. `false` means we were already paused,
    /// so the frontend should fix up state silently rather than toasting.
    pub was_playing: bool,
    /// Post-rebuild state — always paused. Assign straight into the player store.
    pub playback_state: PlaybackState,
}

/// Spawn the watcher thread. It exits when every `Sender<DeviceEvent>` is dropped; in practice
/// the process exits first, which is also how the other watchers in this codebase behave
/// (`follow::watch`, `collection::watch`, `DeviceService::start_monitoring`).
pub(super) fn start(app: AppHandle, audio: AudioService, events: Receiver<DeviceEvent>) {
    // A plain OS thread, not the tokio runtime: cpal enumeration is blocking FFI.
    thread::spawn(move || {
        let mut known = AudioService::get_output_devices().unwrap_or_default();
        let mut last_rebuild: Option<Instant> = None;

        loop {
            match events.recv_timeout(POLL_INTERVAL) {
                Ok(DeviceEvent::StreamLost { epoch }) => {
                    let epoch = drain_losses(&events, epoch);

                    if last_rebuild.is_some_and(|t| t.elapsed() < REBUILD_COOLDOWN) {
                        log::debug!("audio: device lost within rebuild cooldown, ignoring");
                        continue;
                    }

                    match audio.handle_device_lost(epoch) {
                        Ok(Some(payload)) => {
                            last_rebuild = Some(Instant::now());
                            log::info!(
                                "audio: output device '{}' lost; paused and rebuilt on '{}'",
                                payload.lost_device.as_deref().unwrap_or("<unknown>"),
                                payload.new_device.as_deref().unwrap_or("<none>")
                            );
                            let _ = app.emit(DEVICE_LOST_EVENT, &payload);
                        }
                        // Stale epoch or no player — nothing to tell the frontend.
                        Ok(None) => {}
                        Err(e) => log::warn!("audio: device-lost rebuild failed: {e}"),
                    }

                    refresh_devices(&app, &mut known);
                }

                Ok(DeviceEvent::StreamError { epoch }) => {
                    log::warn!("audio: non-fatal stream error on epoch {epoch}");
                }

                Err(RecvTimeoutError::Timeout) => {
                    let Ok(current) = AudioService::get_output_devices() else {
                        continue;
                    };
                    let default_moved = default_name(&current) != default_name(&known);
                    let list_changed = device_names(&current) != device_names(&known);

                    if list_changed || default_moved {
                        known = current;
                        let _ = app.emit(DEVICES_CHANGED_EVENT, &known);
                    }

                    if default_moved {
                        // Only meaningful when following the system default; the command itself
                        // no-ops when a device is pinned or the resolved device is unchanged.
                        if let Err(e) = audio.notify_default_changed() {
                            log::warn!("audio: failed to follow default device change: {e}");
                        }
                    }
                }

                Err(RecvTimeoutError::Disconnected) => {
                    log::info!("audio: device event channel closed, stopping device watcher");
                    break;
                }
            }
        }
    });
}

/// Collapse a burst of loss events into one, keeping the newest generation.
fn drain_losses(events: &Receiver<DeviceEvent>, first: u64) -> u64 {
    let deadline = Instant::now() + LOSS_DEBOUNCE;
    let mut epoch = first;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match events.recv_timeout(remaining) {
            Ok(DeviceEvent::StreamLost { epoch: e }) => epoch = epoch.max(e),
            Ok(DeviceEvent::StreamError { .. }) => {}
            Err(_) => break,
        }
    }
    // Let the OS finish settling on the new default before we ask for it.
    thread::sleep(Duration::from_millis(50));
    epoch
}

fn refresh_devices(app: &AppHandle, known: &mut Vec<AudioDevice>) {
    if let Ok(current) = AudioService::get_output_devices() {
        *known = current;
        let _ = app.emit(DEVICES_CHANGED_EVENT, &*known);
    }
}

fn device_names(devices: &[AudioDevice]) -> Vec<&str> {
    devices.iter().map(|d| d.name.as_str()).collect()
}

fn default_name(devices: &[AudioDevice]) -> Option<&str> {
    devices
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.name.as_str())
}
