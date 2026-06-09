use std::collections::{HashMap, HashSet};
use std::io::{Read as IoRead, Write as IoWrite};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::{params, Connection};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{CrateError, Result};
use crate::models::backup::*;

pub struct BackupService {
    conn: Arc<Mutex<Connection>>,
}

impl BackupService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub fn create_backup_data(&self, app_version: &str) -> Result<BackupData> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Backup("Failed to acquire database lock".into()))?;

        // Tag categories
        let mut stmt = conn.prepare(
            "SELECT id, name, color, sort_order FROM tag_categories ORDER BY sort_order",
        )?;
        let tag_categories = stmt
            .query_map([], |row| {
                Ok(crate::models::TagCategory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    sort_order: row.get(3)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Tags
        let mut stmt = conn.prepare(
            "SELECT id, category_id, name, color, sort_order FROM tags ORDER BY sort_order",
        )?;
        let tags = stmt
            .query_map([], |row| {
                Ok(crate::models::Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Tracks (WITHOUT waveform_data — too large, user re-analyzes after restore)
        let mut stmt = conn.prepare(
            "SELECT id, file_path, file_hash, title, artist, album, year, genre, label,
                    catalog_number, duration_ms, bpm, key, bitrate, sample_rate, format,
                    rating, play_count, date_added, date_modified, last_played,
                    rekordbox_id, artwork_path, artwork_source, color
             FROM tracks",
        )?;
        let tracks = stmt
            .query_map([], |row| {
                Ok(BackupTrack {
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
                    rating: row.get(16)?,
                    play_count: row.get(17)?,
                    date_added: row.get(18)?,
                    date_modified: row.get(19)?,
                    last_played: row.get(20)?,
                    rekordbox_id: row.get(21)?,
                    artwork_path: row.get(22)?,
                    artwork_source: row.get(23)?,
                    color: row.get(24)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Cues
        let mut stmt = conn.prepare(
            "SELECT id, track_id, position_ms, type, loop_end_ms, hot_cue_index, name, color FROM cues",
        )?;
        let cues = stmt
            .query_map([], |row| {
                let cue_type_str: String = row.get(3)?;
                let cue_type = cue_type_str.parse().map_err(|e: String| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?;
                Ok(crate::models::Cue {
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
        drop(stmt);

        // Track tags
        let mut stmt = conn.prepare("SELECT track_id, tag_id FROM track_tags")?;
        let track_tags = stmt
            .query_map([], |row| {
                Ok(BackupTrackTag {
                    track_id: row.get(0)?,
                    tag_id: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Playlists
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, is_folder, is_smart, smart_rules, sort_order,
                    date_created, date_modified, context
             FROM playlists ORDER BY sort_order",
        )?;
        let playlists = stmt
            .query_map([], |row| {
                let is_folder: i32 = row.get(3)?;
                let is_smart: i32 = row.get(4)?;
                Ok(crate::models::Playlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    is_folder: is_folder != 0,
                    is_smart: is_smart != 0,
                    smart_rules: row.get(5)?,
                    sort_order: row.get(6)?,
                    date_created: row.get(7)?,
                    date_modified: row.get(8)?,
                    track_count: 0,
                    context: row
                        .get::<_, Option<String>>(9)?
                        .unwrap_or_else(|| "library".to_string()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Playlist tracks
        let mut stmt = conn
            .prepare("SELECT playlist_id, track_id, position, date_added FROM playlist_tracks")?;
        let playlist_tracks = stmt
            .query_map([], |row| {
                Ok(crate::models::PlaylistTrack {
                    playlist_id: row.get(0)?,
                    track_id: row.get(1)?,
                    position: row.get(2)?,
                    date_added: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Discovery releases
        let mut stmt = conn.prepare(
            "SELECT id, url, source_type, artist, title, label, release_date,
                    artwork_url, artwork_path, notes, parent_url, source_page_url, date_added, date_modified,
                    is_new, surfaced_at
             FROM discovery_releases",
        )?;
        let discovery_releases = stmt
            .query_map([], |row| {
                Ok(crate::models::DiscoveryRelease {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    source_type: row.get(2)?,
                    artist: row.get(3)?,
                    title: row.get(4)?,
                    label: row.get(5)?,
                    release_date: row.get(6)?,
                    artwork_url: row.get(7)?,
                    artwork_path: row.get(8)?,
                    notes: row.get(9)?,
                    parent_url: row.get(10)?,
                    source_page_url: row.get(11)?,
                    date_added: row.get(12)?,
                    date_modified: row.get(13)?,
                    is_new: row.get(14)?,
                    surfaced_at: row.get(15)?,
                    // Provenance is captured via discovery_release_sources, not on the row.
                    source_ids: Vec::new(),
                    tracks: Vec::new(),
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Discovery tracks
        let mut stmt = conn.prepare(
            "SELECT id, release_id, name, position, duration_ms, video_id, is_liked FROM discovery_tracks",
        )?;
        let discovery_tracks = stmt
            .query_map([], |row| {
                Ok(crate::models::DiscoveryTrack {
                    id: row.get(0)?,
                    release_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    duration_ms: row.get(4)?,
                    video_id: row.get(5)?,
                    is_liked: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Discovery release tags
        let mut stmt = conn.prepare("SELECT release_id, tag_id FROM discovery_release_tags")?;
        let discovery_release_tags = stmt
            .query_map([], |row| {
                Ok(BackupDiscoveryReleaseTag {
                    release_id: row.get(0)?,
                    tag_id: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Playlist discovery releases
        let mut stmt = conn.prepare(
            "SELECT playlist_id, release_id, position, date_added FROM playlist_discovery_releases",
        )?;
        let playlist_discovery_releases = stmt
            .query_map([], |row| {
                Ok(BackupPlaylistDiscoveryRelease {
                    playlist_id: row.get(0)?,
                    release_id: row.get(1)?,
                    position: row.get::<_, Option<i32>>(2)?.unwrap_or(0),
                    date_added: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Followed sources (synced follow list)
        let mut stmt = conn.prepare(
            "SELECT id, url, source_type, follow_type, name, artwork_url, artwork_path,
                    enabled, date_added, date_modified
             FROM followed_sources",
        )?;
        let followed_sources = stmt
            .query_map([], |row| {
                Ok(BackupFollowedSource {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    source_type: row.get(2)?,
                    follow_type: row.get(3)?,
                    name: row.get(4)?,
                    artwork_url: row.get(5)?,
                    artwork_path: row.get(6)?,
                    enabled: row.get(7)?,
                    date_added: row.get(8)?,
                    date_modified: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Followed source state (per-device watch bookkeeping)
        let mut stmt = conn.prepare(
            "SELECT source_id, last_checked_at, last_success_at, health, last_error,
                    consecutive_failures, baseline_established
             FROM followed_source_state",
        )?;
        let followed_source_state = stmt
            .query_map([], |row| {
                Ok(BackupFollowedSourceState {
                    source_id: row.get(0)?,
                    last_checked_at: row.get(1)?,
                    last_success_at: row.get(2)?,
                    health: row.get(3)?,
                    last_error: row.get(4)?,
                    consecutive_failures: row.get(5)?,
                    baseline_established: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Followed source releases (seen URLs + dismissal tombstones)
        let mut stmt = conn.prepare(
            "SELECT source_id, seen_url, status, release_id, release_day_notified, first_seen_at
             FROM followed_source_releases",
        )?;
        let followed_source_releases = stmt
            .query_map([], |row| {
                Ok(BackupFollowedSourceRelease {
                    source_id: row.get(0)?,
                    seen_url: row.get(1)?,
                    status: row.get(2)?,
                    release_id: row.get(3)?,
                    release_day_notified: row.get(4)?,
                    first_seen_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // Discovery release sources (provenance m2m)
        let mut stmt =
            conn.prepare("SELECT release_id, source_id FROM discovery_release_sources")?;
        let discovery_release_sources = stmt
            .query_map([], |row| {
                Ok(BackupDiscoveryReleaseSource {
                    release_id: row.get(0)?,
                    source_id: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        let counts = BackupCounts {
            tracks: tracks.len(),
            cues: cues.len(),
            tag_categories: tag_categories.len(),
            tags: tags.len(),
            playlists: playlists.len(),
            discovery_releases: discovery_releases.len(),
            artwork_files: 0,
            followed_sources: followed_sources.len(),
        };

        log::info!(
            "Backup collected: {} tracks, {} cues, {} tag_categories, {} tags, {} playlists, {} discovery_releases",
            counts.tracks, counts.cues, counts.tag_categories, counts.tags, counts.playlists, counts.discovery_releases
        );

        Ok(BackupData {
            version: 1,
            app_version: app_version.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            counts,
            tag_categories,
            tags,
            tracks,
            cues,
            track_tags,
            playlists,
            playlist_tracks,
            discovery_releases,
            discovery_tracks,
            discovery_release_tags,
            playlist_discovery_releases,
            followed_sources,
            followed_source_state,
            followed_source_releases,
            discovery_release_sources,
            artwork_files: None,
        })
    }

    pub fn restore_from_backup_data(&self, data: BackupData) -> Result<()> {
        log::info!(
            "Restore starting with: {} tracks, {} cues, {} tag_categories, {} tags, {} playlists, {} discovery_releases",
            data.counts.tracks, data.counts.cues, data.counts.tag_categories, data.counts.tags, data.counts.playlists, data.counts.discovery_releases
        );

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Backup("Failed to acquire database lock".into()))?;

        conn.execute_batch("PRAGMA foreign_keys = OFF")?;

        let result = (|| -> Result<()> {
            let tx = conn.unchecked_transaction()?;

            // Delete existing data in reverse dependency order (settings NOT touched)
            tx.execute_batch(
                "DELETE FROM discovery_stream_cache;
                 DELETE FROM discovery_audio_cache;
                 DELETE FROM device_exports;
                 DELETE FROM device_tracks;
                 DELETE FROM discovery_release_sources;
                 DELETE FROM followed_source_releases;
                 DELETE FROM followed_source_state;
                 DELETE FROM followed_sources;
                 DELETE FROM playlist_discovery_releases;
                 DELETE FROM discovery_release_tags;
                 DELETE FROM discovery_tracks;
                 DELETE FROM discovery_releases;
                 DELETE FROM playlist_tracks;
                 DELETE FROM track_tags;
                 DELETE FROM cues;
                 DELETE FROM tracks;
                 DELETE FROM tags;
                 DELETE FROM tag_categories;
                 DELETE FROM playlists;",
            )?;

            // Insert in dependency order

            // 1. Tag categories
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO tag_categories (id, name, color, sort_order) VALUES (?1, ?2, ?3, ?4)",
                )?;
                for tc in &data.tag_categories {
                    stmt.execute(params![tc.id, tc.name, tc.color, tc.sort_order])?;
                }
            }

            // 2. Tags
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO tags (id, category_id, name, color, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                )?;
                for t in &data.tags {
                    stmt.execute(params![t.id, t.category_id, t.name, t.color, t.sort_order])?;
                }
            }

            // 3. Tracks (waveform_data and analysis_source set to NULL)
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO tracks (id, file_path, file_hash, title, artist, album, year, genre, label,
                                         catalog_number, duration_ms, bpm, key, bitrate, sample_rate, format,
                                         analysis_source, waveform_data, rating, play_count, date_added,
                                         date_modified, last_played, rekordbox_id, artwork_path, artwork_source, color)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                             NULL, NULL, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
                )?;
                for t in &data.tracks {
                    stmt.execute(params![
                        t.id,
                        t.file_path,
                        t.file_hash,
                        t.title,
                        t.artist,
                        t.album,
                        t.year,
                        t.genre,
                        t.label,
                        t.catalog_number,
                        t.duration_ms,
                        t.bpm,
                        t.key,
                        t.bitrate,
                        t.sample_rate,
                        t.format,
                        t.rating,
                        t.play_count,
                        t.date_added,
                        t.date_modified,
                        t.last_played,
                        t.rekordbox_id,
                        t.artwork_path,
                        t.artwork_source,
                        t.color,
                    ])?;
                }
            }

            // 4. Cues
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO cues (id, track_id, position_ms, type, loop_end_ms, hot_cue_index, name, color)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )?;
                for c in &data.cues {
                    stmt.execute(params![
                        c.id,
                        c.track_id,
                        c.position_ms,
                        c.cue_type.to_string(),
                        c.loop_end_ms,
                        c.hot_cue_index,
                        c.name,
                        c.color,
                    ])?;
                }
            }

            // 5. Track tags
            {
                let mut stmt =
                    tx.prepare("INSERT INTO track_tags (track_id, tag_id) VALUES (?1, ?2)")?;
                for tt in &data.track_tags {
                    stmt.execute(params![tt.track_id, tt.tag_id])?;
                }
            }

            // 6. Playlists (topologically sorted — roots first, then children)
            {
                let sorted = topological_sort_playlists(&data.playlists);
                let mut stmt = tx.prepare(
                    "INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules,
                                            sort_order, context, date_created, date_modified)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )?;
                for p in &sorted {
                    stmt.execute(params![
                        p.id,
                        p.name,
                        p.parent_id,
                        p.is_folder as i32,
                        p.is_smart as i32,
                        p.smart_rules,
                        p.sort_order,
                        p.context,
                        p.date_created,
                        p.date_modified,
                    ])?;
                }
            }

            // 7. Playlist tracks
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO playlist_tracks (playlist_id, track_id, position, date_added) VALUES (?1, ?2, ?3, ?4)",
                )?;
                for pt in &data.playlist_tracks {
                    stmt.execute(params![
                        pt.playlist_id,
                        pt.track_id,
                        pt.position,
                        pt.date_added
                    ])?;
                }
            }

            // 8. Discovery releases
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO discovery_releases (id, url, source_type, artist, title, label, release_date,
                                                      artwork_url, artwork_path, notes, parent_url, source_page_url, date_added, date_modified,
                                                      is_new, surfaced_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                )?;
                for dr in &data.discovery_releases {
                    stmt.execute(params![
                        dr.id,
                        dr.url,
                        dr.source_type,
                        dr.artist,
                        dr.title,
                        dr.label,
                        dr.release_date,
                        dr.artwork_url,
                        dr.artwork_path,
                        dr.notes,
                        dr.parent_url,
                        dr.source_page_url,
                        dr.date_added,
                        dr.date_modified,
                        dr.is_new,
                        dr.surfaced_at,
                    ])?;
                }
            }

            // 9. Discovery tracks
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms, video_id, is_liked)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )?;
                for dt in &data.discovery_tracks {
                    stmt.execute(params![
                        dt.id,
                        dt.release_id,
                        dt.name,
                        dt.position,
                        dt.duration_ms,
                        dt.video_id,
                        dt.is_liked,
                    ])?;
                }
            }

            // 10. Discovery release tags
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO discovery_release_tags (release_id, tag_id) VALUES (?1, ?2)",
                )?;
                for drt in &data.discovery_release_tags {
                    stmt.execute(params![drt.release_id, drt.tag_id])?;
                }
            }

            // 11. Playlist discovery releases
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO playlist_discovery_releases (playlist_id, release_id, position, date_added)
                     VALUES (?1, ?2, ?3, ?4)",
                )?;
                for pdr in &data.playlist_discovery_releases {
                    stmt.execute(params![
                        pdr.playlist_id,
                        pdr.release_id,
                        pdr.position,
                        pdr.date_added,
                    ])?;
                }
            }

            // 12. Followed sources (synced follow list)
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO followed_sources (id, url, source_type, follow_type, name, artwork_url,
                                                   artwork_path, enabled, date_added, date_modified)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )?;
                for fs in &data.followed_sources {
                    stmt.execute(params![
                        fs.id,
                        fs.url,
                        fs.source_type,
                        fs.follow_type,
                        fs.name,
                        fs.artwork_url,
                        fs.artwork_path,
                        fs.enabled,
                        fs.date_added,
                        fs.date_modified,
                    ])?;
                }
            }

            // 13. Followed source state (per-device watch bookkeeping)
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO followed_source_state (source_id, last_checked_at, last_success_at, health,
                                                        last_error, consecutive_failures, baseline_established)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )?;
                for st in &data.followed_source_state {
                    stmt.execute(params![
                        st.source_id,
                        st.last_checked_at,
                        st.last_success_at,
                        st.health,
                        st.last_error,
                        st.consecutive_failures,
                        st.baseline_established,
                    ])?;
                }
            }

            // 14. Followed source releases (seen URLs + dismissal tombstones)
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO followed_source_releases (source_id, seen_url, status, release_id,
                                                           release_day_notified, first_seen_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                for fsr in &data.followed_source_releases {
                    stmt.execute(params![
                        fsr.source_id,
                        fsr.seen_url,
                        fsr.status,
                        fsr.release_id,
                        fsr.release_day_notified,
                        fsr.first_seen_at,
                    ])?;
                }
            }

            // 15. Discovery release sources (provenance m2m — FKs discovery_releases + followed_sources)
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO discovery_release_sources (release_id, source_id) VALUES (?1, ?2)",
                )?;
                for drs in &data.discovery_release_sources {
                    stmt.execute(params![drs.release_id, drs.source_id])?;
                }
            }

            // FK check BEFORE commit — violations trigger rollback
            let mut fk_stmt = tx.prepare("PRAGMA foreign_key_check")?;
            let fk_errors: Vec<String> = fk_stmt
                .query_map([], |row| {
                    let table: String = row.get(0)?;
                    let rowid: i64 = row.get(1)?;
                    Ok(format!("FK violation in {table} row {rowid}"))
                })?
                .filter_map(|r| r.ok())
                .collect();
            drop(fk_stmt);

            if !fk_errors.is_empty() {
                return Err(CrateError::Backup(format!(
                    "Backup contains invalid references: {:?}",
                    &fk_errors[..fk_errors.len().min(5)]
                )));
            }

            tx.commit()?;

            // A wholesale restore replaces the library; force the next cloud-sync
            // push to re-stamp every restored row by clearing the one-time guard.
            let _ = conn.execute(
                "DELETE FROM sync_state WHERE key = 'initial_stamp_done'",
                [],
            );

            // Post-commit verification
            let track_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
            let playlist_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM playlists", [], |r| r.get(0))?;
            let tag_count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))?;
            let discovery_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM discovery_releases", [], |r| r.get(0))?;
            log::info!(
                "Restore post-commit: {} tracks, {} playlists, {} tags, {} discovery_releases in DB",
                track_count, playlist_count, tag_count, discovery_count
            );

            if data.counts.tracks > 0 && track_count == 0 {
                log::error!(
                    "DATA LOSS: Backup had {} tracks but DB shows 0 after commit!",
                    data.counts.tracks
                );
            }

            Ok(())
        })();

        // Always restore FK enforcement, regardless of success/failure
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON");

        result
    }
}

/// Topologically sort playlists so that roots (parent_id = None) come first,
/// then their children, etc. This ensures parent rows exist before children.
fn topological_sort_playlists(
    playlists: &[crate::models::Playlist],
) -> Vec<crate::models::Playlist> {
    use std::collections::{HashMap, VecDeque};

    let mut by_parent: HashMap<Option<&str>, Vec<&crate::models::Playlist>> = HashMap::new();
    for p in playlists {
        by_parent.entry(p.parent_id.as_deref()).or_default().push(p);
    }

    let mut result = Vec::with_capacity(playlists.len());
    let mut queue: VecDeque<Option<&str>> = VecDeque::new();
    queue.push_back(None);

    while let Some(parent) = queue.pop_front() {
        if let Some(children) = by_parent.get(&parent) {
            for child in children {
                result.push((*child).clone());
                queue.push_back(Some(&child.id));
            }
        }
    }

    // If any playlists weren't reached (orphaned), append them at the end
    if result.len() < playlists.len() {
        let in_result: std::collections::HashSet<String> =
            result.iter().map(|p| p.id.clone()).collect();
        for p in playlists {
            if !in_result.contains(&p.id) {
                result.push(p.clone());
            }
        }
    }

    result
}

fn emit_progress(app_handle: &AppHandle, progress: &BackupProgress) {
    let _ = app_handle.emit("backup-progress", progress);
}

pub async fn create_backup(
    path: String,
    conn: Arc<Mutex<Connection>>,
    app_handle: AppHandle,
    app_version: String,
) -> Result<()> {
    let start = Instant::now();

    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::ReadingData,
        },
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mut data = {
        let conn = conn.clone();
        let app_version = app_version.clone();
        tokio::task::spawn_blocking(move || {
            BackupService::new(conn).create_backup_data(&app_version)
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Backup task failed: {e}")))?
    }?;

    // Collect artwork files as base64
    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::CollectingArtwork,
        },
    );

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CrateError::Backup(format!("Failed to resolve app data dir: {e}")))?;

    let artwork_map = collect_artwork_files(&data_dir, &data).await?;
    if !artwork_map.is_empty() {
        data.counts.artwork_files = artwork_map.len();
        data.artwork_files = Some(artwork_map);
    }

    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::WritingFile,
        },
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Serialize to JSON, gzip compress, write to path
    let json = serde_json::to_vec(&data)
        .map_err(|e| CrateError::Backup(format!("Failed to serialize backup: {e}")))?;

    let path_clone = path.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::create(&path_clone)
            .map_err(|e| CrateError::Backup(format!("Failed to create file: {e}")))?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder
            .write_all(&json)
            .map_err(|e| CrateError::Backup(format!("Failed to write backup: {e}")))?;
        encoder
            .finish()
            .map_err(|e| CrateError::Backup(format!("Failed to finalize backup: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| CrateError::Backup(format!("Write task failed: {e}")))??;

    // Save last_backup_at timestamp
    {
        let conn = conn.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn
                .lock()
                .map_err(|_| CrateError::Backup("Failed to acquire database lock".into()))?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('last_backup_at', ?1)",
                params![now],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('last_backup_type', 'manual')",
                [],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Settings save failed: {e}")))??;
    }

    // Ensure minimum 2s total elapsed
    let elapsed = start.elapsed();
    if elapsed < std::time::Duration::from_secs(2) {
        tokio::time::sleep(std::time::Duration::from_secs(2) - elapsed).await;
    }

    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::Completed,
        },
    );

    Ok(())
}

pub async fn restore_from_backup(
    path: String,
    conn: Arc<Mutex<Connection>>,
    app_handle: AppHandle,
) -> Result<()> {
    let start = Instant::now();

    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::Pending,
        },
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Read file, decompress, deserialize
    let mut data: BackupData = {
        let path = path.clone();
        tokio::task::spawn_blocking(move || -> Result<BackupData> {
            let file = std::fs::File::open(&path)
                .map_err(|e| CrateError::Backup(format!("Failed to open backup file: {e}")))?;
            let mut decoder = GzDecoder::new(file);
            let mut json = Vec::new();
            decoder
                .read_to_end(&mut json)
                .map_err(|e| CrateError::Backup(format!("Failed to decompress backup: {e}")))?;
            let data: BackupData = serde_json::from_slice(&json)
                .map_err(|e| CrateError::Backup(format!("Failed to parse backup: {e}")))?;
            Ok(data)
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Read task failed: {e}")))??
    };

    log::info!(
        "Restore deserialized: {} tracks, {} cues, {} tag_categories, {} tags, {} playlists, {} discovery_releases",
        data.counts.tracks, data.counts.cues, data.counts.tag_categories, data.counts.tags, data.counts.playlists, data.counts.discovery_releases
    );

    // Validate version
    if data.version != 1 {
        return Err(CrateError::Backup(format!(
            "Unsupported backup version: {}",
            data.version
        )));
    }

    // Remember expected counts for post-restore verification
    let expected_tracks = data.counts.tracks;
    let expected_playlists = data.counts.playlists;
    let expected_tags = data.counts.tags;

    // Extract artwork data before moving data into DB restore
    let artwork_files = data.artwork_files.take();

    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::RestoringData,
        },
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Restore data
    {
        let conn = conn.clone();
        tokio::task::spawn_blocking(move || BackupService::new(conn).restore_from_backup_data(data))
            .await
            .map_err(|e| CrateError::Backup(format!("Restore task failed: {e}")))?
    }?;

    // Restore artwork files
    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::RestoringArtwork,
        },
    );

    // artwork_path values are relative to app_data_dir (e.g. "artwork/abc.webp")
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CrateError::Backup(format!("Failed to resolve app data dir: {e}")))?;
    let artwork_dir = data_dir.join("artwork");

    if let Some(files) = artwork_files {
        // Backup contains artwork — clear existing artwork dir and write all files
        let data_dir_clone = data_dir.clone();
        let artwork_dir_clone = artwork_dir.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let engine = base64::engine::general_purpose::STANDARD;

            // Clear artwork dir (ignore errors if it doesn't exist)
            if artwork_dir_clone.exists() {
                let _ = std::fs::remove_dir_all(&artwork_dir_clone);
            }
            std::fs::create_dir_all(&artwork_dir_clone)
                .map_err(|e| CrateError::Backup(format!("Failed to create artwork dir: {e}")))?;

            for (rel_path, b64_data) in &files {
                let full_path = data_dir_clone.join(rel_path);
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(bytes) = engine.decode(b64_data) {
                    let _ = std::fs::write(&full_path, bytes);
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Artwork restore failed: {e}")))??;
    }

    // Clean up stale artwork_path references (files that don't exist on disk)
    {
        let conn = conn.clone();
        let data_dir = data_dir.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn
                .lock()
                .map_err(|_| CrateError::Backup("Failed to acquire database lock".into()))?;

            // Clean tracks
            let mut stmt =
                conn.prepare("SELECT id, artwork_path FROM tracks WHERE artwork_path IS NOT NULL")?;
            let stale_track_ids: Vec<String> = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let path: String = row.get(1)?;
                    Ok((id, path))
                })?
                .filter_map(|r| r.ok())
                .filter(|(_, path)| !data_dir.join(path).exists())
                .map(|(id, _)| id)
                .collect();
            drop(stmt);

            if !stale_track_ids.is_empty() {
                let mut update = conn.prepare(
                    "UPDATE tracks SET artwork_path = NULL, artwork_source = NULL WHERE id = ?1",
                )?;
                for id in &stale_track_ids {
                    update.execute(params![id])?;
                }
            }

            // Clean discovery releases
            let mut stmt = conn.prepare(
                "SELECT id, artwork_path FROM discovery_releases WHERE artwork_path IS NOT NULL",
            )?;
            let stale_release_ids: Vec<String> = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let path: String = row.get(1)?;
                    Ok((id, path))
                })?
                .filter_map(|r| r.ok())
                .filter(|(_, path)| !data_dir.join(path).exists())
                .map(|(id, _)| id)
                .collect();
            drop(stmt);

            if !stale_release_ids.is_empty() {
                let mut update = conn
                    .prepare("UPDATE discovery_releases SET artwork_path = NULL WHERE id = ?1")?;
                for id in &stale_release_ids {
                    update.execute(params![id])?;
                }
            }

            Ok(())
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Stale artwork cleanup failed: {e}")))??;
    }

    // Clear cached discovery audio files (they'll be re-fetched on demand)
    let streams_dir = data_dir.join("discovery").join("streams");
    if streams_dir.exists() {
        let _ = std::fs::remove_dir_all(&streams_dir);
    }

    // Post-restore verification (separate lock acquisition)
    {
        let conn = conn.clone();
        let verified = tokio::task::spawn_blocking(move || -> Result<(i64, i64, i64)> {
            let conn = conn.lock().map_err(|_| {
                CrateError::Backup("Failed to acquire database lock for verification".into())
            })?;
            let tracks: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
            let playlists: i64 =
                conn.query_row("SELECT COUNT(*) FROM playlists", [], |r| r.get(0))?;
            let tags: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))?;
            Ok((tracks, playlists, tags))
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Verification task failed: {e}")))??;

        log::info!(
            "Post-restore verification: {} tracks, {} playlists, {} tags",
            verified.0,
            verified.1,
            verified.2
        );

        if expected_tracks > 0 && verified.0 == 0 {
            return Err(CrateError::Backup(
                "Restore failed: backup contained tracks but database is empty after restore. Check logs for details.".into()
            ));
        }
        if expected_playlists > 0 && verified.1 == 0 {
            return Err(CrateError::Backup(
                "Restore failed: backup contained playlists but database is empty after restore. Check logs for details.".into()
            ));
        }
        if expected_tags > 0 && verified.2 == 0 {
            return Err(CrateError::Backup(
                "Restore failed: backup contained tags but database is empty after restore. Check logs for details.".into()
            ));
        }
    }

    // Ensure minimum 2s total elapsed
    let elapsed = start.elapsed();
    if elapsed < std::time::Duration::from_secs(2) {
        tokio::time::sleep(std::time::Duration::from_secs(2) - elapsed).await;
    }

    emit_progress(
        &app_handle,
        &BackupProgress {
            status: BackupStatus::Completed,
        },
    );

    Ok(())
}

pub async fn get_backup_info(path: String) -> Result<BackupInfo> {
    tokio::task::spawn_blocking(move || -> Result<BackupInfo> {
        let file = std::fs::File::open(&path)
            .map_err(|e| CrateError::Backup(format!("Failed to open backup file: {e}")))?;
        let mut decoder = GzDecoder::new(file);
        let mut json = Vec::new();
        decoder
            .read_to_end(&mut json)
            .map_err(|e| CrateError::Backup(format!("Failed to decompress backup: {e}")))?;
        let data: BackupData = serde_json::from_slice(&json)
            .map_err(|e| CrateError::Backup(format!("Failed to parse backup: {e}")))?;
        Ok(BackupInfo {
            version: data.version,
            app_version: data.app_version,
            created_at: data.created_at,
            counts: data.counts,
        })
    })
    .await
    .map_err(|e| CrateError::Backup(format!("Backup info task failed: {e}")))?
}

async fn collect_artwork_files(
    data_dir: &std::path::Path,
    data: &BackupData,
) -> Result<HashMap<String, String>> {
    let mut artwork_paths: HashSet<String> = HashSet::new();
    for t in &data.tracks {
        if let Some(ref p) = t.artwork_path {
            artwork_paths.insert(p.clone());
        }
    }
    for dr in &data.discovery_releases {
        if let Some(ref p) = dr.artwork_path {
            artwork_paths.insert(p.clone());
        }
    }

    if artwork_paths.is_empty() {
        return Ok(HashMap::new());
    }

    let data_dir = data_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> HashMap<String, String> {
        let engine = base64::engine::general_purpose::STANDARD;
        let mut map = HashMap::new();
        for rel_path in artwork_paths {
            let full_path = data_dir.join(&rel_path);
            if let Ok(bytes) = std::fs::read(&full_path) {
                map.insert(rel_path, engine.encode(&bytes));
            }
        }
        map
    })
    .await
    .map_err(|e| CrateError::Backup(format!("Artwork collection failed: {e}")))
}

pub async fn run_auto_backup_if_due(
    conn: Arc<Mutex<Connection>>,
    app_handle: AppHandle,
    app_version: String,
) -> Result<()> {
    use crate::models::BackupFrequency;

    // Read backup_frequency from DB (short-lived lock)
    let frequency: BackupFrequency = {
        let conn = conn
            .lock()
            .map_err(|_| CrateError::Backup("Failed to acquire database lock".into()))?;
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = 'backup_frequency'",
            [],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(v) => v.parse().unwrap_or_default(),
            Err(rusqlite::Error::QueryReturnedNoRows) => BackupFrequency::default(),
            Err(e) => return Err(CrateError::Database(e)),
        }
    };

    if frequency == BackupFrequency::Never {
        log::info!("Auto-backup disabled (frequency=never)");
        return Ok(());
    }

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CrateError::Backup(format!("Failed to resolve app data dir: {e}")))?;
    let auto_backup_path = data_dir.join("crate-auto.cratebackup");

    // Check if backup is due based on the file's modification time
    let threshold = match frequency {
        BackupFrequency::Daily => std::time::Duration::from_secs(24 * 60 * 60),
        BackupFrequency::Weekly => std::time::Duration::from_secs(7 * 24 * 60 * 60),
        BackupFrequency::Monthly => std::time::Duration::from_secs(30 * 24 * 60 * 60),
        BackupFrequency::Never => unreachable!(),
    };

    if auto_backup_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&auto_backup_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) {
                    if elapsed < threshold {
                        log::info!(
                            "Auto-backup not due yet (last: {:.1}h ago, threshold: {:.1}h)",
                            elapsed.as_secs_f64() / 3600.0,
                            threshold.as_secs_f64() / 3600.0,
                        );
                        return Ok(());
                    }
                }
            }
        }
    }

    log::info!("Auto-backup starting (frequency={frequency})");

    // Create backup data
    let mut data = {
        let conn = conn.clone();
        let app_version = app_version.clone();
        tokio::task::spawn_blocking(move || {
            BackupService::new(conn).create_backup_data(&app_version)
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Auto-backup task failed: {e}")))?
    }?;

    // Collect artwork
    let artwork_map = collect_artwork_files(&data_dir, &data).await?;
    if !artwork_map.is_empty() {
        data.counts.artwork_files = artwork_map.len();
        data.artwork_files = Some(artwork_map);
    }

    // Serialize, compress, write
    let json = serde_json::to_vec(&data)
        .map_err(|e| CrateError::Backup(format!("Failed to serialize auto-backup: {e}")))?;

    let path = auto_backup_path.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::create(&path)
            .map_err(|e| CrateError::Backup(format!("Failed to create auto-backup file: {e}")))?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder
            .write_all(&json)
            .map_err(|e| CrateError::Backup(format!("Failed to write auto-backup: {e}")))?;
        encoder
            .finish()
            .map_err(|e| CrateError::Backup(format!("Failed to finalize auto-backup: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| CrateError::Backup(format!("Auto-backup write task failed: {e}")))??;

    // Update last_backup_at and last_backup_type in settings
    {
        let conn = conn.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn
                .lock()
                .map_err(|_| CrateError::Backup("Failed to acquire database lock".into()))?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('last_backup_at', ?1)",
                params![now],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('last_backup_type', 'automatic')",
                [],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| CrateError::Backup(format!("Auto-backup settings save failed: {e}")))??;
    }

    log::info!("Auto-backup completed successfully");
    Ok(())
}
