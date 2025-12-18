use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{Playlist, Track};

pub struct PlaylistService {
    conn: Arc<Mutex<Connection>>,
}

impl PlaylistService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                p.id, p.name, p.parent_id, p.is_folder, p.is_smart,
                p.smart_rules, p.sort_order, p.date_created, p.date_modified,
                COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id), 0) as track_count
            FROM playlists p
            ORDER BY p.sort_order, p.name
            "#,
        )?;

        let playlists = stmt
            .query_map([], |row| {
                Ok(Playlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    is_folder: row.get(3)?,
                    is_smart: row.get(4)?,
                    smart_rules: row.get(5)?,
                    sort_order: row.get(6)?,
                    date_created: row.get(7)?,
                    date_modified: row.get(8)?,
                    track_count: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(playlists)
    }

    pub fn create_playlist(&self, name: String, parent_id: Option<String>) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        // Get next sort order
        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM playlists WHERE parent_id IS ?1",
                rusqlite::params![parent_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let playlist = Playlist {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            parent_id,
            is_folder: false,
            is_smart: false,
            smart_rules: None,
            sort_order: max_order + 1,
            date_created: chrono::Utc::now().to_rfc3339(),
            date_modified: chrono::Utc::now().to_rfc3339(),
            track_count: 0,
        };

        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            rusqlite::params![
                playlist.id,
                playlist.name,
                playlist.parent_id,
                playlist.is_folder,
                playlist.is_smart,
                playlist.smart_rules,
                playlist.sort_order,
                playlist.date_created,
                playlist.date_modified,
            ],
        )?;

        Ok(playlist)
    }

    pub fn create_folder(&self, name: String, parent_id: Option<String>) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM playlists WHERE parent_id IS ?1",
                rusqlite::params![parent_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let folder = Playlist {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            parent_id,
            is_folder: true,
            is_smart: false,
            smart_rules: None,
            sort_order: max_order + 1,
            date_created: chrono::Utc::now().to_rfc3339(),
            date_modified: chrono::Utc::now().to_rfc3339(),
            track_count: 0,
        };

        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            rusqlite::params![
                folder.id,
                folder.name,
                folder.parent_id,
                folder.is_folder,
                folder.is_smart,
                folder.smart_rules,
                folder.sort_order,
                folder.date_created,
                folder.date_modified,
            ],
        )?;

        Ok(folder)
    }

    pub fn rename_playlist(&self, id: &str, name: String) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE playlists SET name = ?1, date_modified = ?2 WHERE id = ?3",
            rusqlite::params![name, now, id],
        )?;

        drop(conn);
        self.get_playlist(id)
    }

    pub fn delete_playlist(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        // Foreign key cascade will delete playlist_tracks entries
        conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;

        Ok(())
    }

    pub fn get_playlist(&self, id: &str) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        conn.query_row(
            r#"
            SELECT
                p.id, p.name, p.parent_id, p.is_folder, p.is_smart,
                p.smart_rules, p.sort_order, p.date_created, p.date_modified,
                COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id), 0) as track_count
            FROM playlists p
            WHERE p.id = ?1
            "#,
            [id],
            |row| {
                Ok(Playlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    is_folder: row.get(3)?,
                    is_smart: row.get(4)?,
                    smart_rules: row.get(5)?,
                    sort_order: row.get(6)?,
                    date_created: row.get(7)?,
                    date_modified: row.get(8)?,
                    track_count: row.get(9)?,
                })
            },
        )
        .map_err(|e| e.into())
    }

    pub fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                t.id, t.file_path, t.file_hash,
                t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                t.analysis_source, t.waveform_data,
                t.rating, t.play_count,
                t.date_added, t.date_modified, t.last_played,
                t.rekordbox_id
            FROM tracks t
            JOIN playlist_tracks pt ON t.id = pt.track_id
            WHERE pt.playlist_id = ?1
            ORDER BY pt.position
            "#,
        )?;

        let tracks = stmt
            .query_map([playlist_id], |row| {
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
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

    pub fn add_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        // Get current max position
        let max_position: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let now = chrono::Utc::now().to_rfc3339();

        for (i, track_id) in track_ids.iter().enumerate() {
            let position = max_position + 1 + i as i32;
            conn.execute(
                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position, date_added) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![playlist_id, track_id, position, now],
            )?;
        }

        // Update playlist modified date
        conn.execute(
            "UPDATE playlists SET date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, playlist_id],
        )?;

        Ok(())
    }

    pub fn remove_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        for track_id in &track_ids {
            conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                rusqlite::params![playlist_id, track_id],
            )?;
        }

        // Reorder remaining tracks
        let mut stmt = conn.prepare(
            "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
        )?;

        let remaining_tracks: Vec<String> = stmt
            .query_map([playlist_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (i, track_id) in remaining_tracks.iter().enumerate() {
            conn.execute(
                "UPDATE playlist_tracks SET position = ?1 WHERE playlist_id = ?2 AND track_id = ?3",
                rusqlite::params![i as i32, playlist_id, track_id],
            )?;
        }

        // Update playlist modified date
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE playlists SET date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, playlist_id],
        )?;

        Ok(())
    }

    pub fn reorder_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        for (i, track_id) in track_ids.iter().enumerate() {
            conn.execute(
                "UPDATE playlist_tracks SET position = ?1 WHERE playlist_id = ?2 AND track_id = ?3",
                rusqlite::params![i as i32, playlist_id, track_id],
            )?;
        }

        // Update playlist modified date
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE playlists SET date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, playlist_id],
        )?;

        Ok(())
    }
}
