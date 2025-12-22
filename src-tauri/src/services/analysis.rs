use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};
use tauri::async_runtime::JoinHandle;
use tokio_util::sync::CancellationToken;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use stratum_dsp::{analyze_audio, AnalysisConfig};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::{CrateError, Result};
use crate::models::{Tag, Track};

/// Result of analyzing a single track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub track_id: String,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

/// Status of an analysis operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Pending,
    Analyzing,
    Completed,
    Failed,
    Cancelled,
}

/// Progress update during analysis operation (legacy - kept for compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisProgress {
    pub status: AnalysisStatus,
    pub current_track_id: Option<String>,
    pub tracks_analyzed: u32,
    pub tracks_total: u32,
    pub result: Option<AnalysisResult>,
    pub updated_track: Option<Track>,
}

/// Per-track analysis event for real-time UI updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAnalysisEvent {
    pub track_id: String,
    pub state: AnalysisStatus,
    pub result: Option<AnalysisResult>,
    pub updated_track: Option<Track>,
    pub error: Option<String>,
}

/// State for a single track's analysis task
struct TrackAnalysisTask {
    cancel_token: CancellationToken,
    #[allow(dead_code)]
    handle: JoinHandle<()>,
}

pub struct AnalysisService {
    conn: Arc<Mutex<Connection>>,
    tasks: Arc<Mutex<HashMap<String, TrackAnalysisTask>>>,
}

impl AnalysisService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Cancel analysis for a specific track
    pub fn cancel_track_analysis(&self, track_id: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.remove(track_id) {
            task.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    /// Cancel all running analysis tasks
    pub fn cancel_all_analysis(&self) {
        let mut tasks = self.tasks.lock().unwrap();
        for (_, task) in tasks.drain() {
            task.cancel_token.cancel();
        }
    }

    /// Analyze multiple tracks with per-track events (async, non-blocking)
    pub async fn analyze_tracks_async(
        &self,
        app_handle: AppHandle,
        track_ids: Vec<String>,
    ) -> Result<()> {
        for track_id in track_ids {
            let cancel_token = CancellationToken::new();
            let conn = self.conn.clone();
            let app = app_handle.clone();
            let tid = track_id.clone();
            let token = cancel_token.clone();
            let tasks = self.tasks.clone();

            // Emit "pending" event immediately
            let _ = app.emit(
                "analysis-track-event",
                TrackAnalysisEvent {
                    track_id: tid.clone(),
                    state: AnalysisStatus::Pending,
                    result: None,
                    updated_track: None,
                    error: None,
                },
            );

            let handle = tauri::async_runtime::spawn(async move {
                Self::analyze_single_track_task(conn, app, tid, token, tasks).await;
            });

            // Store task for potential cancellation
            let mut tasks_guard = self.tasks.lock().unwrap();
            tasks_guard.insert(
                track_id.clone(),
                TrackAnalysisTask {
                    cancel_token,
                    handle,
                },
            );
        }

        Ok(())
    }

    /// Single track analysis task - runs in its own Tokio task
    async fn analyze_single_track_task(
        conn: Arc<Mutex<Connection>>,
        app: AppHandle,
        track_id: String,
        cancel_token: CancellationToken,
        tasks: Arc<Mutex<HashMap<String, TrackAnalysisTask>>>,
    ) {
        // Check if already cancelled before starting
        if cancel_token.is_cancelled() {
            let _ = app.emit(
                "analysis-track-event",
                TrackAnalysisEvent {
                    track_id: track_id.clone(),
                    state: AnalysisStatus::Cancelled,
                    result: None,
                    updated_track: None,
                    error: None,
                },
            );
            tasks.lock().unwrap().remove(&track_id);
            return;
        }

        // Emit "analyzing" status
        let _ = app.emit(
            "analysis-track-event",
            TrackAnalysisEvent {
                track_id: track_id.clone(),
                state: AnalysisStatus::Analyzing,
                result: None,
                updated_track: None,
                error: None,
            },
        );

        // Run analysis on blocking thread pool with cancellation
        let conn_clone = conn.clone();
        let tid_clone = track_id.clone();
        let token_clone = cancel_token.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::analyze_track_with_cancellation(&conn_clone, &tid_clone, &token_clone)
        })
        .await;

        // Check if cancelled during analysis
        if cancel_token.is_cancelled() {
            let _ = app.emit(
                "analysis-track-event",
                TrackAnalysisEvent {
                    track_id: track_id.clone(),
                    state: AnalysisStatus::Cancelled,
                    result: None,
                    updated_track: None,
                    error: None,
                },
            );
            tasks.lock().unwrap().remove(&track_id);
            return;
        }

        // Handle result
        match result {
            Ok(Ok((analysis_result, updated_track))) => {
                let _ = app.emit(
                    "analysis-track-event",
                    TrackAnalysisEvent {
                        track_id: track_id.clone(),
                        state: AnalysisStatus::Completed,
                        result: Some(analysis_result),
                        updated_track,
                        error: None,
                    },
                );
            }
            Ok(Err(e)) => {
                let _ = app.emit(
                    "analysis-track-event",
                    TrackAnalysisEvent {
                        track_id: track_id.clone(),
                        state: AnalysisStatus::Failed,
                        result: None,
                        updated_track: None,
                        error: Some(e.to_string()),
                    },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "analysis-track-event",
                    TrackAnalysisEvent {
                        track_id: track_id.clone(),
                        state: AnalysisStatus::Failed,
                        result: None,
                        updated_track: None,
                        error: Some(format!("Task panicked: {e}")),
                    },
                );
            }
        }

        // Clean up task from map
        tasks.lock().unwrap().remove(&track_id);
    }

    /// Analyze a track with cancellation support - runs on blocking thread
    fn analyze_track_with_cancellation(
        conn: &Arc<Mutex<Connection>>,
        track_id: &str,
        cancel_token: &CancellationToken,
    ) -> Result<(AnalysisResult, Option<Track>)> {
        // Get track from database
        let track = Self::get_track_static(conn, track_id)?;
        let file_path = Path::new(&track.file_path);

        if !file_path.exists() {
            return Ok((
                AnalysisResult {
                    track_id: track_id.to_string(),
                    bpm: None,
                    key: None,
                    success: false,
                    error: Some(format!("File not found: {}", track.file_path)),
                },
                None,
            ));
        }

        // Check cancellation before starting heavy work
        if cancel_token.is_cancelled() {
            return Err(CrateError::Analysis("Cancelled".to_string()));
        }

        // Analyze the audio file with cancellation checks
        match Self::analyze_audio_file_with_cancellation(file_path, cancel_token) {
            Ok((bpm, key)) => {
                // Check cancellation before saving
                if cancel_token.is_cancelled() {
                    return Err(CrateError::Analysis("Cancelled".to_string()));
                }

                // Update the database
                Self::update_track_analysis_static(conn, track_id, bpm, key.as_deref())?;

                // Get updated track
                let updated_track = Self::get_track_static(conn, track_id).ok();

                Ok((
                    AnalysisResult {
                        track_id: track_id.to_string(),
                        bpm,
                        key,
                        success: true,
                        error: None,
                    },
                    updated_track,
                ))
            }
            Err(e) if e.to_string().contains("Cancelled") => {
                Err(CrateError::Analysis("Cancelled".to_string()))
            }
            Err(e) => Ok((
                AnalysisResult {
                    track_id: track_id.to_string(),
                    bpm: None,
                    key: None,
                    success: false,
                    error: Some(e.to_string()),
                },
                None,
            )),
        }
    }

    /// Analyze an audio file for BPM and key with cancellation support
    fn analyze_audio_file_with_cancellation(
        path: &Path,
        cancel_token: &CancellationToken,
    ) -> Result<(Option<f64>, Option<String>)> {
        // Decode audio to mono f32 samples with cancellation checks
        let (samples, sample_rate) = Self::decode_audio_with_cancellation(path, cancel_token)?;

        if samples.is_empty() {
            return Err(CrateError::Analysis("No audio samples found".to_string()));
        }

        // Check cancellation before analysis
        if cancel_token.is_cancelled() {
            return Err(CrateError::Analysis("Cancelled".to_string()));
        }

        // Analyze using stratum-dsp
        let result = analyze_audio(&samples, sample_rate, AnalysisConfig::default())
            .map_err(|e| CrateError::Analysis(format!("Analysis failed: {e}")))?;

        // Round BPM to nearest integer (most tracks are produced at whole BPMs)
        let bpm = Some((result.bpm as f64).round());
        let key = Some(result.key.name().to_string());

        Ok((bpm, key))
    }

    /// Decode audio file to mono f32 samples with cancellation checks
    fn decode_audio_with_cancellation(
        path: &Path,
        cancel_token: &CancellationToken,
    ) -> Result<(Vec<f32>, u32)> {
        let file = File::open(path).map_err(|e| CrateError::Analysis(e.to_string()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| CrateError::Analysis(format!("Failed to probe audio: {e}")))?;

        let mut format = probed.format;

        let track = format
            .default_track()
            .ok_or_else(|| CrateError::Analysis("No audio track found".to_string()))?;

        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| CrateError::Analysis("Unknown sample rate".to_string()))?;

        let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

        let track_id = track.id;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| CrateError::Analysis(format!("Failed to create decoder: {e}")))?;

        let mut samples: Vec<f32> = Vec::new();
        let mut packet_count: u32 = 0;

        // Decode all packets with cancellation checks
        loop {
            // Check cancellation every 100 packets
            packet_count += 1;
            if packet_count % 100 == 0 && cancel_token.is_cancelled() {
                return Err(CrateError::Analysis("Cancelled".to_string()));
            }

            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => {
                    log::warn!("Error reading packet: {e}");
                    break;
                }
            };

            // Skip packets from other tracks
            if packet.track_id() != track_id {
                continue;
            }

            let decoded = match decoder.decode(&packet) {
                Ok(d) => d,
                Err(e) => {
                    log::warn!("Error decoding packet: {e}");
                    continue;
                }
            };

            // Convert to f32 samples
            let spec = *decoded.spec();
            let num_frames = decoded.frames();

            let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
            sample_buf.copy_interleaved_ref(decoded);

            let interleaved = sample_buf.samples();

            // Convert to mono by averaging channels
            for chunk in interleaved.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                samples.push(mono);
            }
        }

        Ok((samples, sample_rate))
    }

    /// Static version of update_track_analysis for use in blocking context
    fn update_track_analysis_static(
        conn: &Arc<Mutex<Connection>>,
        track_id: &str,
        bpm: Option<f64>,
        key: Option<&str>,
    ) -> Result<()> {
        let conn = conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE tracks SET bpm = ?1, key = ?2, analysis_source = 'crate', date_modified = ?3 WHERE id = ?4",
            rusqlite::params![bpm, key, now, track_id],
        )?;

        Ok(())
    }

    /// Static version of get_track for use in blocking context
    fn get_track_static(conn: &Arc<Mutex<Connection>>, id: &str) -> Result<Track> {
        let conn = conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, file_path, file_hash,
                   title, artist, album, year, genre, label, catalog_number,
                   duration_ms, bpm, key, bitrate, sample_rate, format,
                   analysis_source, waveform_data,
                   rating, play_count,
                   date_added, date_modified, last_played,
                   rekordbox_id, artwork_path, artwork_source, color
            FROM tracks
            WHERE id = ?1
            "#,
        )?;

        let mut track = stmt.query_row([id], |row| {
            Ok(Track {
                id: row.get(0)?,
                file_path: row.get(1)?,
                file_hash: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                year: row.get(6)?,
                genre: row.get(7)?,
                label: row.get(8)?,
                catalog_number: row.get(9)?,
                duration_ms: row.get(10)?,
                bpm: row.get(11)?,
                key: row.get(12)?,
                bitrate: row.get(13)?,
                sample_rate: row.get(14)?,
                format: row.get(15)?,
                analysis_source: row.get(16)?,
                waveform_data: row.get(17)?,
                rating: row.get(18)?,
                play_count: row.get(19)?,
                date_added: row.get(20)?,
                date_modified: row.get(21)?,
                last_played: row.get(22)?,
                rekordbox_id: row.get(23)?,
                artwork_path: row.get(24)?,
                artwork_source: row.get(25)?,
                color: row.get(26)?,
                tags: Vec::new(),
            })
        })?;

        // Fetch tags
        Self::fetch_tags_for_track_static(&conn, &mut track)?;
        Ok(track)
    }

    /// Static version of fetch_tags for use in blocking context
    fn fetch_tags_for_track_static(conn: &Connection, track: &mut Track) -> Result<()> {
        let mut stmt = conn.prepare(
            r#"
            SELECT t.id, t.category_id, t.name, t.color, t.sort_order
            FROM track_tags tt
            JOIN tags t ON tt.tag_id = t.id
            WHERE tt.track_id = ?1
            "#,
        )?;

        let tags = stmt
            .query_map([&track.id], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        track.tags = tags;
        Ok(())
    }

    // =========================================================================
    // Legacy methods for backward compatibility
    // =========================================================================

    /// Legacy: Cancel the current analysis operation (cancels all)
    pub fn cancel_analysis(&self) {
        self.cancel_all_analysis();
    }

    /// Legacy: Analyze a single track and update the database
    pub fn analyze_track(&self, track_id: &str) -> Result<AnalysisResult> {
        let track = self.get_track(track_id)?;
        let file_path = Path::new(&track.file_path);

        if !file_path.exists() {
            return Ok(AnalysisResult {
                track_id: track_id.to_string(),
                bpm: None,
                key: None,
                success: false,
                error: Some(format!("File not found: {}", track.file_path)),
            });
        }

        match self.analyze_audio_file(file_path) {
            Ok((bpm, key)) => {
                self.update_track_analysis(track_id, bpm, key.as_deref())?;

                Ok(AnalysisResult {
                    track_id: track_id.to_string(),
                    bpm,
                    key,
                    success: true,
                    error: None,
                })
            }
            Err(e) => Ok(AnalysisResult {
                track_id: track_id.to_string(),
                bpm: None,
                key: None,
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Legacy: Analyze multiple tracks with progress events (blocking)
    pub fn analyze_tracks(
        &self,
        app_handle: &AppHandle,
        track_ids: Vec<String>,
    ) -> Result<Vec<AnalysisResult>> {
        let total = track_ids.len() as u32;
        let mut results = Vec::new();

        // Emit initial pending status
        let _ = app_handle.emit(
            "analysis-progress",
            AnalysisProgress {
                status: AnalysisStatus::Pending,
                current_track_id: None,
                tracks_analyzed: 0,
                tracks_total: total,
                result: None,
                updated_track: None,
            },
        );

        for (index, track_id) in track_ids.iter().enumerate() {
            // Emit "analyzing" status before starting this track
            let _ = app_handle.emit(
                "analysis-progress",
                AnalysisProgress {
                    status: AnalysisStatus::Analyzing,
                    current_track_id: Some(track_id.clone()),
                    tracks_analyzed: index as u32,
                    tracks_total: total,
                    result: None,
                    updated_track: None,
                },
            );

            // Analyze the track
            let result = self.analyze_track(track_id)?;

            // Get updated track if analysis succeeded
            let updated_track = if result.success {
                self.get_track(track_id).ok()
            } else {
                None
            };

            // Emit progress with result and updated track
            let _ = app_handle.emit(
                "analysis-progress",
                AnalysisProgress {
                    status: AnalysisStatus::Analyzing,
                    current_track_id: Some(track_id.clone()),
                    tracks_analyzed: (index + 1) as u32,
                    tracks_total: total,
                    result: Some(result.clone()),
                    updated_track,
                },
            );

            results.push(result);
        }

        // Emit completion
        let _ = app_handle.emit(
            "analysis-progress",
            AnalysisProgress {
                status: AnalysisStatus::Completed,
                current_track_id: None,
                tracks_analyzed: results.len() as u32,
                tracks_total: total,
                result: None,
                updated_track: None,
            },
        );

        Ok(results)
    }

    /// Legacy: Analyze an audio file for BPM and key
    fn analyze_audio_file(&self, path: &Path) -> Result<(Option<f64>, Option<String>)> {
        let (samples, sample_rate) = self.decode_audio(path)?;

        if samples.is_empty() {
            return Err(CrateError::Analysis("No audio samples found".to_string()));
        }

        let result = analyze_audio(&samples, sample_rate, AnalysisConfig::default())
            .map_err(|e| CrateError::Analysis(format!("Analysis failed: {e}")))?;

        let bpm = Some((result.bpm as f64).round());
        let key = Some(result.key.name().to_string());

        Ok((bpm, key))
    }

    /// Legacy: Decode audio file to mono f32 samples
    fn decode_audio(&self, path: &Path) -> Result<(Vec<f32>, u32)> {
        let file = File::open(path).map_err(|e| CrateError::Analysis(e.to_string()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| CrateError::Analysis(format!("Failed to probe audio: {e}")))?;

        let mut format = probed.format;

        let track = format
            .default_track()
            .ok_or_else(|| CrateError::Analysis("No audio track found".to_string()))?;

        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| CrateError::Analysis("Unknown sample rate".to_string()))?;

        let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

        let track_id = track.id;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| CrateError::Analysis(format!("Failed to create decoder: {e}")))?;

        let mut samples: Vec<f32> = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => {
                    log::warn!("Error reading packet: {e}");
                    break;
                }
            };

            if packet.track_id() != track_id {
                continue;
            }

            let decoded = match decoder.decode(&packet) {
                Ok(d) => d,
                Err(e) => {
                    log::warn!("Error decoding packet: {e}");
                    continue;
                }
            };

            let spec = *decoded.spec();
            let num_frames = decoded.frames();

            let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
            sample_buf.copy_interleaved_ref(decoded);

            let interleaved = sample_buf.samples();

            for chunk in interleaved.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                samples.push(mono);
            }
        }

        Ok((samples, sample_rate))
    }

    /// Legacy: Update track analysis results in the database
    fn update_track_analysis(
        &self,
        track_id: &str,
        bpm: Option<f64>,
        key: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE tracks SET bpm = ?1, key = ?2, analysis_source = 'crate', date_modified = ?3 WHERE id = ?4",
            rusqlite::params![bpm, key, now, track_id],
        )?;

        Ok(())
    }

    /// Legacy: Get a track by ID
    fn get_track(&self, id: &str) -> Result<Track> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, file_path, file_hash,
                   title, artist, album, year, genre, label, catalog_number,
                   duration_ms, bpm, key, bitrate, sample_rate, format,
                   analysis_source, waveform_data,
                   rating, play_count,
                   date_added, date_modified, last_played,
                   rekordbox_id, artwork_path, artwork_source, color
            FROM tracks
            WHERE id = ?1
            "#,
        )?;

        let track = stmt.query_row([id], |row| {
            Ok(Track {
                id: row.get(0)?,
                file_path: row.get(1)?,
                file_hash: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                year: row.get(6)?,
                genre: row.get(7)?,
                label: row.get(8)?,
                catalog_number: row.get(9)?,
                duration_ms: row.get(10)?,
                bpm: row.get(11)?,
                key: row.get(12)?,
                bitrate: row.get(13)?,
                sample_rate: row.get(14)?,
                format: row.get(15)?,
                analysis_source: row.get(16)?,
                waveform_data: row.get(17)?,
                rating: row.get(18)?,
                play_count: row.get(19)?,
                date_added: row.get(20)?,
                date_modified: row.get(21)?,
                last_played: row.get(22)?,
                rekordbox_id: row.get(23)?,
                artwork_path: row.get(24)?,
                artwork_source: row.get(25)?,
                color: row.get(26)?,
                tags: Vec::new(),
            })
        })?;

        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, vec![track])?;
        Ok(tracks_with_tags.into_iter().next().unwrap())
    }

    fn fetch_tags_for_tracks(
        &self,
        conn: &Connection,
        mut tracks: Vec<Track>,
    ) -> Result<Vec<Track>> {
        if tracks.is_empty() {
            return Ok(tracks);
        }

        let track_ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
        let placeholders: Vec<String> = track_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();

        let sql = format!(
            r#"
            SELECT tt.track_id, t.id, t.category_id, t.name, t.color, t.sort_order
            FROM track_tags tt
            JOIN tags t ON tt.tag_id = t.id
            WHERE tt.track_id IN ({})
            "#,
            placeholders.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = track_ids
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let mut stmt = conn.prepare(&sql)?;
        let tag_rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Tag {
                        id: row.get(1)?,
                        category_id: row.get(2)?,
                        name: row.get(3)?,
                        color: row.get(4)?,
                        sort_order: row.get(5)?,
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut tags_by_track: std::collections::HashMap<String, Vec<Tag>> =
            std::collections::HashMap::new();
        for (track_id, tag) in tag_rows {
            tags_by_track.entry(track_id).or_default().push(tag);
        }

        for track in &mut tracks {
            if let Some(tags) = tags_by_track.remove(&track.id) {
                track.tags = tags;
            }
        }

        Ok(tracks)
    }

    /// Get updated track after analysis (for returning to frontend)
    pub fn get_updated_track(&self, track_id: &str) -> Result<Track> {
        self.get_track(track_id)
    }
}

impl Clone for AnalysisService {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            tasks: self.tasks.clone(),
        }
    }
}
