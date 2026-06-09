use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::*;

impl ExportService {
    /// Generate the Rekordbox PDB file
    pub(super) fn generate_rekordbox_pdb(
        &self,
        mount_point: &str,
        playlists_with_tracks: &[(Playlist, Vec<Track>)],
        device_tracks: &[DeviceTrack],
    ) -> Result<()> {
        let pdb_path = Path::new(mount_point)
            .join("PIONEER")
            .join("rekordbox")
            .join("export.pdb");

        // Check if existing PDB exists for merging
        let existing_pdb = if pdb_path.exists() {
            Some(
                fs::read(&pdb_path)
                    .map_err(|e| CrateError::Device(format!("Failed to read existing PDB: {e}")))?,
            )
        } else {
            None
        };

        // Create PDB writer
        let mut writer = if let Some(ref data) = existing_pdb {
            RekordboxPdbWriter::from_existing(data)?
        } else {
            RekordboxPdbWriter::new()
        };

        // Build track ID to USB path mapping
        let track_paths: HashMap<String, String> = device_tracks
            .iter()
            .map(|dt| (dt.track_id.clone(), dt.usb_path.clone()))
            .collect();

        // Track PDB ID counter for ANLZ path generation
        let mut next_pdb_id: u32 = 1;

        // Add tracks to PDB
        let mut track_pdb_ids: HashMap<String, u32> = HashMap::new();
        for (_, tracks) in playlists_with_tracks {
            for track in tracks {
                if let Some(usb_path) = track_paths.get(&track.id) {
                    if !track_pdb_ids.contains_key(&track.id) {
                        // Generate ANLZ files for this track (.DAT, .EXT, .2EX)
                        let anlz_path = self.generate_anlz_file(
                            mount_point,
                            next_pdb_id,
                            usb_path,
                            track.duration_ms as u32,
                            track.bpm.map(|b| b as f32),
                            &track.id,
                        )?;

                        let pdb_id = writer.add_track(track, usb_path, &anlz_path);
                        track_pdb_ids.insert(track.id.clone(), pdb_id);
                        next_pdb_id += 1;
                    }
                }
            }
        }

        // Add playlists to PDB
        for (playlist, tracks) in playlists_with_tracks {
            let track_ids: Vec<u32> = tracks
                .iter()
                .filter_map(|t| track_pdb_ids.get(&t.id).copied())
                .collect();
            writer.add_playlist(playlist, &track_ids);
        }

        // Write PDB file
        writer.write(&pdb_path)?;

        // Update device_tracks with PDB IDs
        self.update_device_track_pdb_ids(&track_pdb_ids)?;

        Ok(())
    }

    /// Generate the Device Library Plus database (SQLCipher encrypted SQLite)
    pub(super) fn generate_device_library_plus(
        &self,
        mount_point: &str,
        playlists_with_tracks: &[(Playlist, Vec<Track>)],
        device_tracks: &[DeviceTrack],
    ) -> Result<()> {
        let db_path = Path::new(mount_point)
            .join("PIONEER")
            .join("rekordbox")
            .join("exportLibrary.db");

        // Create the Device Library Plus writer (creates encrypted database)
        let mut writer = DeviceLibraryPlusWriter::create(&db_path)?;

        // Build track ID to USB path mapping
        let track_paths: HashMap<String, String> = device_tracks
            .iter()
            .map(|dt| (dt.track_id.clone(), dt.usb_path.clone()))
            .collect();

        // Track content ID mapping (Crate track ID -> Device Library Plus content ID)
        let mut content_ids: HashMap<String, i64> = HashMap::new();
        let mut next_content_id: u32 = 1;

        // Add tracks to database
        for (_, tracks) in playlists_with_tracks {
            for track in tracks {
                if let Some(usb_path) = track_paths.get(&track.id) {
                    if content_ids.contains_key(&track.id) {
                        continue;
                    }

                    // Generate ANLZ files for this track (.DAT, .EXT, .2EX)
                    let anlz_path = self.generate_anlz_file(
                        mount_point,
                        next_content_id,
                        usb_path,
                        track.duration_ms as u32,
                        track.bpm.map(|b| b as f32),
                        &track.id,
                    )?;

                    // Build content entry
                    let content_id = self.add_track_to_device_library_plus(
                        &mut writer,
                        track,
                        usb_path,
                        &anlz_path,
                    )?;

                    content_ids.insert(track.id.clone(), content_id);
                    next_content_id += 1;
                }
            }
        }

        // Add playlists to database
        let mut playlist_ids: HashMap<String, i64> = HashMap::new();
        let mut seq_no = 1;

        for (playlist, tracks) in playlists_with_tracks {
            // Determine playlist type
            let playlist_type = if playlist.is_folder {
                PlaylistType::Folder
            } else if playlist.is_smart {
                PlaylistType::SmartPlaylist
            } else {
                PlaylistType::Playlist
            };

            // Create playlist entry
            let mut dlp_playlist = DlpPlaylist::new(playlist.name.clone(), seq_no, playlist_type);

            // Set parent if exists
            if let Some(ref parent_id) = playlist.parent_id {
                if let Some(&parent_dlp_id) = playlist_ids.get(parent_id) {
                    dlp_playlist = dlp_playlist.with_parent(parent_dlp_id);
                }
            }

            let dlp_playlist_id = writer.add_playlist(&dlp_playlist, &playlist.id)?;
            playlist_ids.insert(playlist.id.clone(), dlp_playlist_id);

            // Add tracks to playlist
            let mut track_seq = 1;
            for track in tracks {
                if let Some(&content_id) = content_ids.get(&track.id) {
                    writer.add_playlist_content(dlp_playlist_id, content_id, track_seq)?;
                    track_seq += 1;
                }
            }

            seq_no += 1;
        }

        // Set database property
        let now = chrono::Utc::now();
        let created_date = now.format("%Y-%m-%d %H:%M:%S%.3f +00:00").to_string();
        let mut property = Property::new("Crate Export".to_string());
        property.created_date = Some(created_date);
        writer.set_property(&property)?;
        writer.update_content_count()?;

        // Commit changes
        writer.commit()?;

        // Update device_tracks with content IDs
        let track_pdb_ids: HashMap<String, u32> = content_ids
            .iter()
            .map(|(k, v)| (k.clone(), *v as u32))
            .collect();
        self.update_device_track_pdb_ids(&track_pdb_ids)?;

        Ok(())
    }

    /// Add a track to the Device Library Plus database
    pub(super) fn add_track_to_device_library_plus(
        &self,
        writer: &mut DeviceLibraryPlusWriter,
        track: &Track,
        usb_path: &str,
        anlz_path: &str,
    ) -> Result<i64> {
        // Get or create artist
        let artist_id = if let Some(ref artist) = track.artist {
            Some(writer.get_or_create_artist(artist)?)
        } else {
            None
        };

        // Get or create album
        let album_id = if let Some(ref album) = track.album {
            Some(writer.get_or_create_album(album, artist_id)?)
        } else {
            None
        };

        // Get or create genre
        let genre_id = if let Some(ref genre) = track.genre {
            Some(writer.get_or_create_genre(genre)?)
        } else {
            None
        };

        // Get or create key
        let key_id = if let Some(ref key) = track.key {
            Some(writer.get_or_create_key(key)?)
        } else {
            None
        };

        // Get or create color
        let color_id = if let Some(ref color) = track.color {
            Some(writer.get_or_create_color(color)?)
        } else {
            None
        };

        // Get or create label
        let label_id = if let Some(ref label) = track.label {
            Some(writer.get_or_create_label(label)?)
        } else {
            None
        };

        // Determine file type from format
        let file_type = FileType::from_extension(&track.format)
            .map(|ft| ft as i32)
            .unwrap_or(1); // Default to MP3

        // Build file path on device
        let device_path = format!("/Contents/{usb_path}");
        let file_name = Path::new(usb_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Get file size
        let file_size = fs::metadata(&track.file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        // Build content entry
        let mut content = Content::new(
            device_path,
            file_name,
            file_size,
            file_type,
            track.bitrate.unwrap_or(0),
            16, // Default bit depth
            track.sample_rate.unwrap_or(44100),
        );

        // Set metadata
        content.title = track.title.clone();
        content.title_for_search = track.title.as_ref().map(|t| t.to_lowercase());
        content.bpmx100 = track.bpm.map(|b| (b * 100.0) as i32);
        content.length = Some((track.duration_ms / 1000) as i32);
        content.artist_id_artist = artist_id;
        content.album_id = album_id;
        content.genre_id = genre_id;
        content.key_id = key_id;
        content.color_id = color_id;
        content.label_id = label_id;
        content.rating = Some(track.rating);
        content.release_year = track.year;
        content.analysis_data_file_path = Some(anlz_path.to_string());

        // Set dates
        let now = chrono::Utc::now();
        let date_str = now.format("%Y-%m-%d %H:%M:%S%.3f +00:00").to_string();
        content.date_added = Some(date_str.clone());
        content.date_created = Some(date_str);

        // Add content to database
        let content_id = writer.add_content(&content)?;

        // Add cue points
        let cues = self.get_track_cues(&track.id)?;
        for cue in cues {
            let kind = match cue.cue_type {
                CueType::Memory => CueKind::Cue,
                CueType::Hot => CueKind::Cue,
                CueType::Loop => CueKind::Loop,
            };

            let mut dlp_cue = DlpCue::new(content_id, kind, cue.position_ms * 1000);
            dlp_cue.cue_comment = cue.name;
            dlp_cue.color_table_index = cue.hot_cue_index;

            if cue.cue_type == CueType::Loop {
                if let Some(loop_end) = cue.loop_end_ms {
                    dlp_cue.out_usec = Some(loop_end * 1000);
                    dlp_cue.is_active_loop = Some(1);
                }
            }

            writer.add_cue(&dlp_cue)?;
        }

        Ok(content_id)
    }

    /// Get all cue points for a track
    pub(super) fn get_track_cues(&self, track_id: &str) -> Result<Vec<Cue>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, track_id, position_ms, type, loop_end_ms, hot_cue_index, name, color
            FROM cues
            WHERE track_id = ?1
            ORDER BY position_ms
            "#,
        )?;

        let cues = stmt
            .query_map([track_id], |row| {
                let cue_type_str: String = row.get(3)?;
                let cue_type = cue_type_str.parse::<CueType>().unwrap_or(CueType::Memory);

                Ok(Cue {
                    id: row.get(0)?,
                    track_id: row.get(1)?,
                    position_ms: row.get(2)?,
                    cue_type,
                    loop_end_ms: row.get(4)?,
                    hot_cue_index: row.get(5)?,
                    name: row.get(6)?,
                    color: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(cues)
    }

    /// Generate ANLZ files for a track and return the device path
    pub(super) fn generate_anlz_file(
        &self,
        mount_point: &str,
        pdb_track_id: u32,
        usb_audio_path: &str,
        duration_ms: u32,
        bpm: Option<f32>,
        track_id: &str,
    ) -> Result<String> {
        // Fetch cues for this track
        let cues = self.get_track_cues(track_id)?;

        // Build device audio path
        let device_audio_path = format!("/Contents/{usb_audio_path}");

        // Write all ANLZ variants (.DAT, .EXT, .2EX) using the new module
        let anlz_path = anlz::write_anlz_files(
            mount_point,
            pdb_track_id,
            &device_audio_path,
            duration_ms,
            bpm,
            &cues,
        )?;

        Ok(anlz_path)
    }
}
