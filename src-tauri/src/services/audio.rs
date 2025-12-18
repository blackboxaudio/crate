use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lofty::file::AudioFile;
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};

use crate::error::{CrateError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
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
    GetState,
    #[allow(dead_code)]
    Shutdown,
}

#[derive(Debug)]
enum AudioResponse {
    State(PlaybackState),
    Error(String),
    Ok,
}

pub struct AudioService {
    command_tx: Sender<AudioCommand>,
    response_rx: Arc<Mutex<Receiver<AudioResponse>>>,
}

impl AudioService {
    pub fn new() -> Result<Self> {
        let (command_tx, command_rx) = mpsc::channel::<AudioCommand>();
        let (response_tx, response_rx) = mpsc::channel::<AudioResponse>();

        // Spawn audio thread
        thread::spawn(move || {
            audio_thread(command_rx, response_tx);
        });

        Ok(Self {
            command_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
        })
    }

    fn send_command(&self, cmd: AudioCommand) -> Result<AudioResponse> {
        self.command_tx
            .send(cmd)
            .map_err(|e| CrateError::Audio(format!("Failed to send command: {}", e)))?;

        let rx = self.response_rx.lock().map_err(|_| {
            CrateError::Audio("Failed to acquire response lock".to_string())
        })?;

        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|e| CrateError::Audio(format!("Failed to receive response: {}", e)))
    }

    pub fn play_track(&self, track_id: String, file_path: PathBuf) -> Result<PlaybackState> {
        if !file_path.exists() {
            return Err(CrateError::FileNotFound(file_path));
        }

        // Get duration
        let tagged_file = lofty::read_from_path(&file_path)
            .map_err(|e| CrateError::Audio(e.to_string()))?;
        let duration_ms = tagged_file.properties().duration().as_millis() as u64;

        match self.send_command(AudioCommand::Play {
            track_id,
            file_path,
            duration_ms,
        })? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }

    pub fn pause(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Pause)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }

    pub fn resume(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Resume)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }

    pub fn stop(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Stop)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }

    pub fn seek(&self, position_ms: u64) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::Seek(position_ms))? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }

    pub fn set_volume(&self, volume: f32) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::SetVolume(volume))? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }

    pub fn get_state(&self) -> Result<PlaybackState> {
        match self.send_command(AudioCommand::GetState)? {
            AudioResponse::State(state) => Ok(state),
            AudioResponse::Error(e) => Err(CrateError::Audio(e)),
            AudioResponse::Ok => Ok(PlaybackState::default()),
        }
    }
}

impl Clone for AudioService {
    fn clone(&self) -> Self {
        Self {
            command_tx: self.command_tx.clone(),
            response_rx: self.response_rx.clone(),
        }
    }
}

struct AudioPlayer {
    _stream: OutputStream,
    sink: Sink,
    state: PlaybackState,
}

fn audio_thread(command_rx: Receiver<AudioCommand>, response_tx: Sender<AudioResponse>) {
    let mut player: Option<AudioPlayer> = None;
    let mut current_volume: f32 = 1.0;

    loop {
        match command_rx.recv() {
            Ok(cmd) => {
                let response = handle_command(cmd, &mut player, &mut current_volume);

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

fn handle_command(
    cmd: AudioCommand,
    player: &mut Option<AudioPlayer>,
    current_volume: &mut f32,
) -> AudioResponse {
    match cmd {
        AudioCommand::Play {
            track_id,
            file_path,
            duration_ms,
        } => {
            // Stop any existing playback
            *player = None;

            // Create new audio output
            let (stream, stream_handle) = match OutputStream::try_default() {
                Ok(s) => s,
                Err(e) => return AudioResponse::Error(format!("Failed to create audio output: {}", e)),
            };

            let sink = match Sink::try_new(&stream_handle) {
                Ok(s) => s,
                Err(e) => return AudioResponse::Error(format!("Failed to create sink: {}", e)),
            };

            // Open and decode file
            let file = match File::open(&file_path) {
                Ok(f) => f,
                Err(e) => return AudioResponse::Error(format!("Failed to open file: {}", e)),
            };

            let reader = BufReader::new(file);
            let source = match Decoder::new(reader) {
                Ok(s) => s,
                Err(e) => return AudioResponse::Error(format!("Failed to decode audio: {}", e)),
            };

            sink.append(source);
            sink.set_volume(*current_volume);

            let state = PlaybackState {
                is_playing: true,
                position_ms: 0,
                duration_ms,
                volume: *current_volume,
                current_track_id: Some(track_id),
                current_track_path: Some(file_path.to_string_lossy().to_string()),
            };

            *player = Some(AudioPlayer {
                _stream: stream,
                sink,
                state: state.clone(),
            });

            AudioResponse::State(state)
        }

        AudioCommand::Pause => {
            if let Some(ref mut p) = player {
                p.sink.pause();
                p.state.is_playing = false;
                AudioResponse::State(p.state.clone())
            } else {
                AudioResponse::State(PlaybackState::default())
            }
        }

        AudioCommand::Resume => {
            if let Some(ref mut p) = player {
                p.sink.play();
                p.state.is_playing = true;
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
                ..Default::default()
            })
        }

        AudioCommand::Seek(position_ms) => {
            if let Some(ref mut p) = player {
                let _ = p.sink.try_seek(Duration::from_millis(position_ms));
                p.state.position_ms = position_ms;
                AudioResponse::State(p.state.clone())
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
                    ..Default::default()
                })
            }
        }

        AudioCommand::GetState => {
            if let Some(ref p) = player {
                let mut state = p.state.clone();
                state.is_playing = !p.sink.is_paused() && !p.sink.empty();
                AudioResponse::State(state)
            } else {
                AudioResponse::State(PlaybackState {
                    volume: *current_volume,
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
