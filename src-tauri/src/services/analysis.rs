use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};

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
use crate::models::Track;

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

/// Progress update during analysis operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisProgress {
    pub status: AnalysisStatus,
    pub current_track_id: Option<String>,
    pub tracks_analyzed: u32,
    pub tracks_total: u32,
    pub result: Option<AnalysisResult>,
    pub updated_track: Option<Track>,
}

pub struct AnalysisService {
    conn: Arc<Mutex<Connection>>,
    cancel_flag: Arc<AtomicBool>,
}

impl AnalysisService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancel the current analysis operation
    pub fn cancel_analysis(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Analyze a single track and update the database
    pub fn analyze_track(&self, track_id: &str) -> Result<AnalysisResult> {
        // Get track from database
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

        // Analyze the audio file
        match self.analyze_audio_file(file_path) {
            Ok((bpm, key)) => {
                // Update the database
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

    /// Analyze multiple tracks with progress events
    pub fn analyze_tracks(
        &self,
        app_handle: &AppHandle,
        track_ids: Vec<String>,
    ) -> Result<Vec<AnalysisResult>> {
        // Reset cancel flag
        self.cancel_flag.store(false, Ordering::SeqCst);

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
            // Check for cancellation
            if self.cancel_flag.load(Ordering::SeqCst) {
                let _ = app_handle.emit(
                    "analysis-progress",
                    AnalysisProgress {
                        status: AnalysisStatus::Cancelled,
                        current_track_id: None,
                        tracks_analyzed: index as u32,
                        tracks_total: total,
                        result: None,
                        updated_track: None,
                    },
                );
                break;
            }

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

        // Emit completion (only if not cancelled)
        if !self.cancel_flag.load(Ordering::SeqCst) {
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
        }

        Ok(results)
    }

    /// Analyze an audio file for BPM and key using stratum-dsp
    fn analyze_audio_file(&self, path: &Path) -> Result<(Option<f64>, Option<String>)> {
        // Decode audio to mono f32 samples
        let (samples, sample_rate) = self.decode_audio(path)?;

        if samples.is_empty() {
            return Err(CrateError::Analysis("No audio samples found".to_string()));
        }

        // Analyze using stratum-dsp
        let result = analyze_audio(&samples, sample_rate, AnalysisConfig::default())
            .map_err(|e| CrateError::Analysis(format!("Analysis failed: {e}")))?;

        // Round BPM to nearest integer (most tracks are produced at whole BPMs)
        let bpm = Some((result.bpm as f64).round());
        let key = Some(result.key.name().to_string());

        Ok((bpm, key))
    }

    /// Decode audio file to mono f32 samples
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

        // Decode all packets
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

    /// Update track analysis results in the database
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

    /// Get a track by ID
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

        Ok(track)
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
            cancel_flag: self.cancel_flag.clone(),
        }
    }
}
