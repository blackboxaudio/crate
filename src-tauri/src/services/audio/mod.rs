mod device_watch;
mod fade;

pub use device_watch::DeviceLostPayload;

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lofty::config::{ParseOptions, ParsingMode};
use lofty::file::AudioFile;
use lofty::probe::Probe;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{stream::OutputStreamBuilder, Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

use crate::error::{CrateError, Result};
use crate::models::AudioDevice;

use device_watch::DeviceEvent;
use fade::{FadeOutEnding, FadeState, PauseFade};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub speed: f32,
    pub current_track_id: Option<String>,
    pub current_track_path: Option<String>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            speed: 1.0,
            current_track_id: None,
            current_track_path: None,
        }
    }
}

#[derive(Debug)]
enum AudioCommand {
    Play {
        track_id: String,
        file_path: PathBuf,
        duration_ms: u64,
    },
    Pause,
    Resume,
    Stop,
    Seek(u64),
    SetVolume(f32),
    SetSpeed(f32),
    SetDevice(Option<String>),
    /// The output device backing stream generation `epoch` disappeared. Rebuild on the
    /// current default and leave the sink PAUSED at the live position.
    HandleDeviceLost {
        epoch: u64,
    },
    /// The system default output device changed while we were following it
    /// (`selected_device == None`). Rebuild on the new default, preserving play/pause.
    DefaultDeviceChanged,
    GetState,
    #[allow(dead_code)]
    Shutdown,
}

#[derive(Debug)]
enum AudioResponse {
    State(PlaybackState),
    /// `None` = the event was stale (a newer stream already replaced that generation) or
    /// there was no player — nothing to tell the frontend.
    DeviceLost(Box<Option<DeviceLostPayload>>),
    Error(String),
    Ok,
}

pub struct AudioService {
    command_tx: Sender<AudioCommand>,
    response_rx: Arc<Mutex<Receiver<AudioResponse>>>,
    /// Consumed by `start_device_monitoring`. Held here rather than returned from `new`
    /// so the constructor signature stays unchanged for its single caller in `lib.rs`.
    device_event_rx: Arc<Mutex<Option<Receiver<DeviceEvent>>>>,
}

impl AudioService {
    pub fn new() -> Result<Self> {
        let (command_tx, command_rx) = mpsc::channel::<AudioCommand>();
        let (response_tx, response_rx) = mpsc::channel::<AudioResponse>();
        let (device_event_tx, device_event_rx) = mpsc::channel::<DeviceEvent>();

        // Spawn audio thread
        thread::spawn(move || {
            audio_thread(command_rx, response_tx, device_event_tx);
        });

        Ok(Self {
            command_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
            device_event_rx: Arc::new(Mutex::new(Some(device_event_rx))),
        })
    }

    /// Spawn the output-device watcher (pause-on-disconnect + device-list refresh).
    /// Idempotent — the receiver is consumed on the first call.
    pub fn start_device_monitoring(&self, app_handle: tauri::AppHandle) {
        let Ok(mut slot) = self.device_event_rx.lock() else {
            log::warn!("audio: device event receiver lock poisoned; monitoring not started");
            return;
        };
        let Some(events) = slot.take() else {
            return;
        };
        device_watch::start(app_handle, self.clone(), events);
    }

    /// Rebuild the stream after the device backing `epoch` vanished, leaving it paused.
    /// Called only from the device watcher thread.
    fn handle_device_lost(&self, epoch: u64) -> Result<Option<DeviceLostPayload>> {
        match self.send_command(AudioCommand::HandleDeviceLost { epoch })? {
            AudioResponse::DeviceLost(payload) => Ok(*payload),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(None),
        }
    }

    /// Follow the system default output onto its new device. No-op when a device is pinned.
    fn notify_default_changed(&self) -> Result<()> {
        self.send_command(AudioCommand::DefaultDeviceChanged)?;
        Ok(())
    }

    fn send_command(&self, cmd: AudioCommand) -> Result<AudioResponse> {
        // Take the response lock BEFORE sending so each command/response pair is atomic.
        // Sending first would let two concurrent callers pick up each other's response —
        // harmless while every command came from the UI thread, but the device watcher
        // issues commands from its own thread.
        let rx = self
            .response_rx
            .lock()
            .map_err(|_| CrateError::Audio("Failed to acquire response lock".to_string()))?;

        self.command_tx
            .send(cmd)
            .map_err(|e| CrateError::Audio(format!("Failed to send command: {e}")))?;

        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|e| CrateError::Audio(format!("Failed to receive response: {e}")))
    }

    pub fn play_track(&self, track_id: String, file_path: PathBuf) -> Result<PlaybackState> {
        if !file_path.exists() {
            return Err(CrateError::FileNotFound(file_path));
        }

        // Get duration using lenient parsing, with symphonia fallback
        let duration_ms = self.get_track_duration(&file_path)?;

        match self.send_command(AudioCommand::Play {
            track_id,
            file_path,
            duration_ms,
        })? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    /// Gets track duration, trying lofty with lenient options first, then symphonia fallback.
    fn get_track_duration(&self, path: &PathBuf) -> Result<u64> {
        // Try lofty with lenient options
        if let Some(tagged_file) = Self::read_metadata_lenient(path) {
            return Ok(tagged_file.properties().duration().as_millis() as u64);
        }

        // Fall back to symphonia
        log::warn!(
            "Lofty failed for {}, falling back to symphonia for duration",
            path.display()
        );

        let file =
            File::open(path).map_err(|e| CrateError::Audio(format!("Failed to open: {e}")))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &Default::default(), &Default::default())
            .map_err(|e| CrateError::Audio(format!("Failed to probe audio: {e}")))?;

        let track = probed
            .format
            .default_track()
            .ok_or_else(|| CrateError::Audio("No audio track found".to_string()))?;

        match (track.codec_params.n_frames, track.codec_params.sample_rate) {
            (Some(frames), Some(sample_rate)) => {
                Ok(((frames as f64 / sample_rate as f64) * 1000.0) as u64)
            }
            _ => Ok(0), // Duration unknown, player can still work
        }
    }

    /// Attempts to read audio file metadata with lenient parsing options.
    fn read_metadata_lenient(path: &PathBuf) -> Option<lofty::file::TaggedFile> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);

        let parse_options = ParseOptions::new()
            .parsing_mode(ParsingMode::Relaxed)
            .max_junk_bytes(4096);

        Probe::new(reader)
            .options(parse_options)
            .guess_file_type()
            .ok()?
            .read()
            .ok()
    }

    pub fn pause(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Pause)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn resume(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Resume)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn stop(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Stop)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn seek(&self, position_ms: u64) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Seek(position_ms))? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn set_volume(&self, volume: f32) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::SetVolume(volume))? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn set_speed(&self, speed: f32) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::SetSpeed(speed))? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn get_state(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::GetState)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(PlaybackState::default()),
        }
    }

    pub fn set_device(&self, device_name: Option<String>) -> Result<()> {
        match self.send_command(AudioCommand::SetDevice(device_name))? {
            AudioResponse::Ok => Ok(()),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            _ => Ok(()),
        }
    }

    pub fn get_output_devices() -> Result<Vec<AudioDevice>> {
        let host = rodio::cpal::default_host();
        let default_device = host.default_output_device();
        let default_name = default_device.as_ref().and_then(|d| d.name().ok());

        let devices = host
            .output_devices()
            .map_err(|e| CrateError::Audio(format!("Failed to enumerate devices: {e}")))?;

        let mut result = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                let is_default = default_name.as_ref() == Some(&name);
                let is_built_in = Self::is_built_in_device(&name);
                result.push(AudioDevice {
                    name,
                    is_default,
                    is_built_in,
                });
            }
        }

        Ok(result)
    }

    /// Determines if an audio device is a built-in/system device based on name heuristics.
    fn is_built_in_device(name: &str) -> bool {
        let name_lower = name.to_lowercase();

        #[cfg(target_os = "macos")]
        {
            name_lower.contains("built-in")
                || name_lower.contains("macbook")
                || name_lower.contains("internal")
                || (name_lower.contains("speakers") && !name_lower.contains("bluetooth"))
                || name_lower.starts_with("mac")
        }

        #[cfg(target_os = "windows")]
        {
            name_lower.contains("speakers")
                || name_lower.contains("realtek")
                || name_lower.contains("intel")
                || name_lower.contains("high definition audio")
                || name_lower.contains("built-in")
                || name_lower.contains("internal")
                || name_lower.contains("conexant")
                || name_lower.contains("synaptics")
        }

        #[cfg(target_os = "linux")]
        {
            name_lower.contains("built-in")
                || name_lower.contains("internal")
                || name_lower.contains("hda intel")
                || (name_lower.contains("analog stereo") && !name_lower.contains("usb"))
                || name_lower.contains("pch")
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            name_lower.contains("built-in") || name_lower.contains("internal")
        }
    }
}

impl Clone for AudioService {
    fn clone(&self) -> Self {
        Self {
            command_tx: self.command_tx.clone(),
            response_rx: self.response_rx.clone(),
            device_event_rx: self.device_event_rx.clone(),
        }
    }
}

struct AudioPlayer {
    _stream: OutputStream,
    sink: Sink,
    state: PlaybackState,
    fade_state: Arc<AtomicU8>,
    speed: f32,
    // For tracking playback position
    started_at: Option<Instant>,
    started_position_ms: u64,
    /// Name of the device the stream is ACTUALLY open on, resolved at creation time.
    /// This is what makes a `selected_device: None` player comparable against the system
    /// default — `selected_device` alone says "follow the default", not *which* device that
    /// resolved to. Also records the silent fallback when a pinned device is missing.
    /// `None` only when the name couldn't be read.
    active_device: Option<String>,
    /// Monotonic id of this stream generation, carried by the cpal error callback so a
    /// callback fired by an already-replaced stream is discarded instead of pausing the new one.
    epoch: u64,
}

impl AudioPlayer {
    /// Calculate the current playback position based on elapsed time
    fn get_current_position_ms(&self) -> u64 {
        if let Some(started) = self.started_at {
            if !self.sink.is_paused() && !self.sink.empty() {
                let elapsed = started.elapsed().as_millis() as u64;
                let position =
                    self.started_position_ms + (elapsed as f64 * self.speed as f64) as u64;
                // Don't exceed duration
                return position.min(self.state.duration_ms);
            }
        }
        self.state.position_ms
    }
}

fn audio_thread(
    command_rx: Receiver<AudioCommand>,
    response_tx: Sender<AudioResponse>,
    device_events: Sender<DeviceEvent>,
) {
    let mut player: Option<AudioPlayer> = None;
    let mut current_volume: f32 = 1.0;
    let mut current_speed: f32 = 1.0;
    let mut selected_device: Option<String> = None;
    let mut next_epoch: u64 = 0;

    loop {
        match command_rx.recv() {
            Ok(cmd) => {
                let response = handle_command(
                    cmd,
                    &mut player,
                    &mut current_volume,
                    &mut current_speed,
                    &mut selected_device,
                    &mut next_epoch,
                    &device_events,
                );

                // Check if we should shutdown
                if matches!(response, AudioResponse::Ok) && player.is_none() {
                    // This was a shutdown command - but we'll keep running
                }

                if response_tx.send(response).is_err() {
                    log::error!("Audio response channel closed");
                    break;
                }
            }
            Err(_) => {
                log::info!("Audio command channel closed, shutting down audio thread");
                break;
            }
        }
    }
}

fn find_device_by_name(name: &str) -> Option<rodio::cpal::Device> {
    let host = rodio::cpal::default_host();
    host.output_devices()
        .ok()?
        .find(|d| d.name().ok().as_deref() == Some(name))
}

fn default_device_name() -> Option<String> {
    rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
}

/// Open an output stream and return it alongside the *resolved* device name.
///
/// The error callback attached here is the whole device-loss detector: cpal already
/// registers a native `kAudioDevicePropertyDeviceIsAlive` listener per stream on macOS (and
/// maps WASAPI's `AUDCLNT_E_DEVICE_INVALIDATED` on Windows) and reports the loss as
/// [`rodio::cpal::StreamError::DeviceNotAvailable`]. Without a callback rodio swallows it in
/// its default `eprintln!` handler, which is why a dead Bluetooth device used to leave the
/// sink reporting `is_playing: true` forever.
fn create_output_stream(
    selected_device: &Option<String>,
    epoch: u64,
    events: &Sender<DeviceEvent>,
) -> std::result::Result<(OutputStream, Option<String>), String> {
    // SAFETY-CRITICAL: this closure runs on a CoreAudio HAL notification thread (macOS) or the
    // WASAPI run loop (Windows), *while cpal holds its stream mutex*. It must do nothing but
    // send and return — a blocking `send_command` here would deadlock against the audio thread
    // dropping the stream (which removes this very listener and frees the boxed callback).
    let make_callback = |epoch: u64| {
        let tx = events.clone();
        move |err: rodio::cpal::StreamError| {
            let _ = tx.send(match err {
                rodio::cpal::StreamError::DeviceNotAvailable => DeviceEvent::StreamLost { epoch },
                _ => DeviceEvent::StreamError { epoch },
            });
        }
    };

    if let Some(ref device_name) = selected_device {
        if let Some(device) = find_device_by_name(device_name) {
            match OutputStreamBuilder::from_device(device)
                .map(|b| b.with_error_callback(make_callback(epoch)))
                .and_then(|b| b.open_stream())
            {
                Ok(s) => return Ok((s, Some(device_name.clone()))),
                Err(e) => {
                    log::warn!(
                        "Failed to use device '{device_name}': {e}, falling back to default"
                    );
                }
            }
        } else {
            log::warn!("Device '{device_name}' not found, falling back to default");
        }
    }

    // Default path. This deliberately does NOT use `OutputStreamBuilder::open_default_stream`,
    // which hard-codes rodio's own error callback and would drop the detector; instead it
    // replicates that helper's two-tier fallback with our callback attached.
    let resolved = default_device_name();
    match OutputStreamBuilder::from_default_device()
        .map(|b| b.with_error_callback(make_callback(epoch)))
        .and_then(|b| b.open_stream())
    {
        Ok(s) => return Ok((s, resolved)),
        Err(e) => log::warn!("Failed to open default output device: {e}, trying others"),
    }

    // Last resort: any device that will open. `StreamError::NoDevice` from the call above is
    // the non-panicking "there is no output device at all" signal, and this loop yields nothing.
    let host = rodio::cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate devices: {e}"))?;
    for device in devices {
        let name = device.name().ok();
        if let Ok(stream) = OutputStreamBuilder::from_device(device)
            .map(|b| b.with_error_callback(make_callback(epoch)))
            .and_then(|b| b.open_stream())
        {
            return Ok((stream, name));
        }
    }

    Err("No audio output device available".to_string())
}

#[allow(clippy::too_many_arguments)]
fn create_player(
    track_id: String,
    file_path: &str,
    duration_ms: u64,
    position_ms: u64,
    is_playing: bool,
    volume: f32,
    speed: f32,
    selected_device: &Option<String>,
    epoch: u64,
    events: &Sender<DeviceEvent>,
) -> std::result::Result<AudioPlayer, String> {
    let (stream, active_device) = create_output_stream(selected_device, epoch, events)?;
    let sink = Sink::connect_new(stream.mixer());

    let file = File::open(file_path).map_err(|e| format!("Failed to open file: {e}"))?;
    let reader = BufReader::new(file);
    let source = Decoder::new(reader).map_err(|e| format!("Failed to decode audio: {e}"))?;

    let fade_state = Arc::new(AtomicU8::new(FadeState::Playing as u8));
    let source = FadeOutEnding::new(source, duration_ms);
    let source = PauseFade::new(source, Arc::clone(&fade_state));
    sink.append(source);

    if position_ms > 0 {
        let _ = sink.try_seek(Duration::from_millis(position_ms));
    }

    sink.set_volume(volume);
    sink.set_speed(speed);

    if !is_playing {
        sink.pause();
    }

    let state = PlaybackState {
        is_playing,
        position_ms,
        duration_ms,
        volume,
        speed,
        current_track_id: Some(track_id),
        current_track_path: Some(file_path.to_string()),
    };

    Ok(AudioPlayer {
        _stream: stream,
        sink,
        state,
        fade_state,
        speed,
        started_at: if is_playing {
            Some(Instant::now())
        } else {
            None
        },
        started_position_ms: position_ms,
        active_device,
        epoch,
    })
}

/// Rebuild the current player's stream in place, preserving track identity and seeking back to
/// `position_ms`. Bumps the stream generation so any in-flight error callback from the old
/// stream is discarded. The old player is dropped *before* the new stream is opened, so the
/// dead device's audio unit and its property listener are released first.
#[allow(clippy::too_many_arguments)]
fn rebuild_player(
    player: &mut Option<AudioPlayer>,
    position_ms: u64,
    is_playing: bool,
    volume: f32,
    speed: f32,
    selected_device: &Option<String>,
    next_epoch: &mut u64,
    events: &Sender<DeviceEvent>,
) -> std::result::Result<(), String> {
    let Some(p) = player.as_ref() else {
        return Ok(());
    };
    let duration_ms = p.state.duration_ms;
    let (Some(id), Some(path)) = (
        p.state.current_track_id.clone(),
        p.state.current_track_path.clone(),
    ) else {
        return Ok(());
    };

    *player = None;
    *next_epoch += 1;

    let new_player = create_player(
        id,
        &path,
        duration_ms,
        position_ms,
        is_playing,
        volume,
        speed,
        selected_device,
        *next_epoch,
        events,
    )?;
    *player = Some(new_player);
    Ok(())
}

/// Polls the atomic fade state at 1ms intervals, returning `true` if the
/// target state is reached within the timeout.
fn wait_for_fade_state(fade_state: &AtomicU8, target: FadeState, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if FadeState::from_u8(fade_state.load(Ordering::Relaxed)) == target {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_command(
    cmd: AudioCommand,
    player: &mut Option<AudioPlayer>,
    current_volume: &mut f32,
    current_speed: &mut f32,
    selected_device: &mut Option<String>,
    next_epoch: &mut u64,
    device_events: &Sender<DeviceEvent>,
) -> AudioResponse {
    match cmd {
        AudioCommand::Play {
            track_id,
            file_path,
            duration_ms,
        } => {
            *player = None;
            *next_epoch += 1;

            match create_player(
                track_id,
                &file_path.to_string_lossy(),
                duration_ms,
                0,
                true,
                *current_volume,
                *current_speed,
                selected_device,
                *next_epoch,
                device_events,
            ) {
                Ok(p) => {
                    let state = p.state.clone();
                    *player = Some(p);
                    AudioResponse::State(state)
                }
                Err(e) => AudioResponse::Error(e),
            }
        }

        AudioCommand::Pause => {
            if let Some(ref mut p) = player {
                // Save current position before pausing
                p.state.position_ms = p.get_current_position_ms();
                p.started_position_ms = p.state.position_ms;
                p.started_at = None;

                // Trigger source-level fade-out and wait for silence
                p.fade_state
                    .store(FadeState::FadingOut as u8, Ordering::Relaxed);
                if !wait_for_fade_state(&p.fade_state, FadeState::Silent, Duration::from_millis(50))
                {
                    log::warn!("Pause fade-out timed out, forcing pause");
                }
                p.sink.pause();

                p.state.is_playing = false;
                AudioResponse::State(p.state.clone())
            } else {
                AudioResponse::State(PlaybackState::default())
            }
        }

        AudioCommand::Resume => {
            if let Some(ref mut p) = player {
                // Trigger source-level fade-in, then unpause — first samples
                // will be near-silent and ramp up smoothly in the audio callback
                p.fade_state
                    .store(FadeState::FadingIn as u8, Ordering::Relaxed);
                p.sink.play();

                p.state.is_playing = true;
                // Restart the timer from current position
                p.started_at = Some(Instant::now());
                AudioResponse::State(p.state.clone())
            } else {
                AudioResponse::State(PlaybackState::default())
            }
        }

        AudioCommand::Stop => {
            if let Some(ref mut p) = player {
                p.sink.stop();
            }
            *player = None;
            AudioResponse::State(PlaybackState {
                volume: *current_volume,
                speed: *current_speed,
                ..Default::default()
            })
        }

        AudioCommand::Seek(position_ms) => {
            if let Some(ref mut p) = player {
                // Fade out before seeking to avoid audio click from sample discontinuity
                if !p.sink.is_paused() {
                    p.fade_state
                        .store(FadeState::FadingOut as u8, Ordering::Relaxed);
                    if !wait_for_fade_state(
                        &p.fade_state,
                        FadeState::Silent,
                        Duration::from_millis(50),
                    ) {
                        log::warn!("Seek fade-out timed out");
                    }
                }

                match p.sink.try_seek(Duration::from_millis(position_ms)) {
                    Ok(()) => {
                        p.state.position_ms = position_ms;
                        p.started_position_ms = position_ms;
                        if !p.sink.is_paused() {
                            p.started_at = Some(Instant::now());
                        }
                        AudioResponse::State(p.state.clone())
                    }
                    Err(e) => {
                        log::warn!("Seek failed ({e}), rebuilding player at {position_ms}ms");

                        let track_id = p.state.current_track_id.clone().unwrap_or_default();
                        let file_path = p.state.current_track_path.clone().unwrap_or_default();
                        let duration_ms = p.state.duration_ms;
                        let is_playing = !p.sink.is_paused() && !p.sink.empty();
                        *next_epoch += 1;

                        match create_player(
                            track_id,
                            &file_path,
                            duration_ms,
                            position_ms,
                            is_playing,
                            *current_volume,
                            *current_speed,
                            selected_device,
                            *next_epoch,
                            device_events,
                        ) {
                            Ok(new_player) => {
                                let state = new_player.state.clone();
                                *player = Some(new_player);
                                AudioResponse::State(state)
                            }
                            Err(rebuild_err) => {
                                log::error!("Failed to rebuild player after seek: {rebuild_err}");
                                AudioResponse::Error(rebuild_err)
                            }
                        }
                    }
                }
            } else {
                AudioResponse::State(PlaybackState::default())
            }
        }

        AudioCommand::SetVolume(volume) => {
            let clamped = volume.clamp(0.0, 1.0);
            *current_volume = clamped;

            if let Some(ref mut p) = player {
                p.sink.set_volume(clamped);
                p.state.volume = clamped;
                AudioResponse::State(p.state.clone())
            } else {
                AudioResponse::State(PlaybackState {
                    volume: clamped,
                    speed: *current_speed,
                    ..Default::default()
                })
            }
        }

        AudioCommand::SetSpeed(speed) => {
            let clamped = speed.clamp(0.9, 1.1);
            *current_speed = clamped;

            if let Some(ref mut p) = player {
                // Recalculate position anchor to prevent jump
                let current_pos = p.get_current_position_ms();
                p.started_position_ms = current_pos;
                p.state.position_ms = current_pos;
                if p.started_at.is_some() {
                    p.started_at = Some(Instant::now());
                }

                p.sink.set_speed(clamped);
                p.speed = clamped;
                p.state.speed = clamped;
                AudioResponse::State(p.state.clone())
            } else {
                AudioResponse::State(PlaybackState {
                    speed: clamped,
                    volume: *current_volume,
                    ..Default::default()
                })
            }
        }

        AudioCommand::SetDevice(device_name) => {
            *selected_device = device_name;

            // Snapshot first so the shared borrow of `player` ends before `rebuild_player`
            // takes it mutably.
            let snapshot = player.as_ref().map(|p| {
                (
                    !p.sink.is_paused() && !p.sink.empty(),
                    p.get_current_position_ms(),
                )
            });

            if let Some((is_playing, current_pos)) = snapshot {
                if let Err(e) = rebuild_player(
                    player,
                    current_pos,
                    is_playing,
                    *current_volume,
                    *current_speed,
                    selected_device,
                    next_epoch,
                    device_events,
                ) {
                    log::warn!("Failed to switch device: {e}");
                }
            }

            AudioResponse::Ok
        }

        AudioCommand::HandleDeviceLost { epoch } => {
            let Some(p) = player.as_ref() else {
                return AudioResponse::DeviceLost(Box::new(None));
            };
            // A callback from a stream we have already replaced — the current stream is
            // healthy, so pausing it here would be the bug this guard exists to prevent.
            if p.epoch != epoch {
                log::debug!("audio: stale device-lost for epoch {epoch}, ignoring");
                return AudioResponse::DeviceLost(Box::new(None));
            }

            let lost_device = p.active_device.clone();
            let was_playing = !p.sink.is_paused() && !p.sink.empty();
            let position_ms = p.get_current_position_ms();
            let track_id = p.state.current_track_id.clone();
            let track_path = p.state.current_track_path.clone();
            let duration_ms = p.state.duration_ms;

            // No fade-out: the device is already gone (cpal stopped the audio unit before
            // calling us), so waiting on the fade would stall the audio thread for nothing.

            // Rebuild PAUSED. `create_player` with `is_playing: false` pauses the sink,
            // sets `state.is_playing = false`, and leaves `started_at` unset — while still
            // seeking to `position_ms` — so `GetState` correctly reports paused and a later
            // Resume plays from where the audio actually stopped.
            if let Err(e) = rebuild_player(
                player,
                position_ms,
                false,
                *current_volume,
                *current_speed,
                selected_device,
                next_epoch,
                device_events,
            ) {
                // No usable output device at all. Don't panic and don't lose the track:
                // report a synthesized paused state carrying the track identity so the UI
                // keeps showing it instead of blanking.
                log::warn!("audio: no output device after loss ({e}); playback stopped");
                return AudioResponse::DeviceLost(Box::new(Some(DeviceLostPayload {
                    lost_device,
                    new_device: None,
                    was_playing,
                    playback_state: PlaybackState {
                        is_playing: false,
                        position_ms,
                        duration_ms,
                        volume: *current_volume,
                        speed: *current_speed,
                        current_track_id: track_id,
                        current_track_path: track_path,
                    },
                })));
            }

            let (new_device, playback_state) = match player.as_ref() {
                Some(np) => (np.active_device.clone(), np.state.clone()),
                None => (None, PlaybackState::default()),
            };
            AudioResponse::DeviceLost(Box::new(Some(DeviceLostPayload {
                lost_device,
                new_device,
                was_playing,
                playback_state,
            })))
        }

        AudioCommand::DefaultDeviceChanged => {
            // An explicitly pinned device must not be yanked away because the OS default moved.
            if selected_device.is_some() {
                return AudioResponse::Ok;
            }
            let Some(p) = player.as_ref() else {
                return AudioResponse::Ok;
            };
            if default_device_name() == p.active_device {
                return AudioResponse::Ok;
            }

            // The old device is still alive — this is a route change, not a loss, so keep
            // playing and just follow the default onto its new device.
            let is_playing = !p.sink.is_paused() && !p.sink.empty();
            let position_ms = p.get_current_position_ms();
            if let Err(e) = rebuild_player(
                player,
                position_ms,
                is_playing,
                *current_volume,
                *current_speed,
                selected_device,
                next_epoch,
                device_events,
            ) {
                log::warn!("audio: failed to follow default device change: {e}");
            }

            AudioResponse::Ok
        }

        AudioCommand::GetState => {
            if let Some(ref p) = player {
                let mut state = p.state.clone();
                state.is_playing = !p.sink.is_paused() && !p.sink.empty();
                AudioResponse::State(state)
            } else {
                AudioResponse::State(PlaybackState {
                    volume: *current_volume,
                    speed: *current_speed,
                    ..Default::default()
                })
            }
        }

        AudioCommand::Shutdown => {
            *player = None;
            AudioResponse::Ok
        }
    }
}
