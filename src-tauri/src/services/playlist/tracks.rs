use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl PlaylistService {
    pub fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                t.id, t.file_path, t.file_hash,
                t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                t.analysis_source, t.waveform_data,
                t.rating, t.play_count,
                t.date_added, t.date_modified, t.last_played,
                t.rekordbox_id, t.artwork_path, t.artwork_source, t.color,
                t.library_root_id, t.relative_path
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
                    artwork_path: row.get(24)?,
                    artwork_source: row.get(25)?,
                    color: row.get(26)?,
                    library_root_id: row.get(27)?,
                    relative_path: row.get(28)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Fetch tags
        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, tracks)?;
        Ok(tracks_with_tags)
    }

    pub(crate) fn fetch_tags_for_tracks(
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

        // Group tags by track_id
        let mut tags_by_track: std::collections::HashMap<String, Vec<Tag>> =
            std::collections::HashMap::new();
        for (track_id, tag) in tag_rows {
            tags_by_track.entry(track_id).or_default().push(tag);
        }

        // Assign tags to tracks
        for track in &mut tracks {
            if let Some(tags) = tags_by_track.remove(&track.id) {
                track.tags = tags;
            }
        }

        Ok(tracks)
    }

    pub fn add_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        // Get current max position
        let max_position: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;

        for (i, track_id) in track_ids.iter().enumerate() {
            let position = max_position + 1 + i as i32;
            conn.execute(
                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position, date_added, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![playlist_id, track_id, position, now, hlc],
            )?;
        }

        // Update playlist modified date
        conn.execute(
            "UPDATE playlists SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, playlist_id],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        // Drop the lock before calling get_playlist which acquires its own lock
        drop(conn);

        // Return the updated playlist with accurate track count
        self.get_playlist(playlist_id)
    }

    pub fn remove_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for track_id in &track_ids {
            let deleted = conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                rusqlite::params![playlist_id, track_id],
            )?;
            if deleted > 0 {
                dirty::record_tombstone(
                    &conn,
                    buckets::PLAYLIST_TRACKS,
                    &dirty::junction_entity_id(playlist_id, track_id),
                    &hlc,
                )?;
            }
        }

        // Reorder remaining tracks
        let remaining_tracks: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
            )?;
            let tracks = stmt
                .query_map([playlist_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            tracks
        };

        for (i, track_id) in remaining_tracks.iter().enumerate() {
            conn.execute(
                "UPDATE playlist_tracks SET position = ?1, _hlc = ?2 WHERE playlist_id = ?3 AND track_id = ?4",
                rusqlite::params![i as i32, hlc, playlist_id, track_id],
            )?;
        }

        // Update playlist modified date
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE playlists SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, playlist_id],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        // Drop the lock before calling get_playlist which acquires its own lock
        drop(conn);

        // Return the updated playlist with accurate track count
        self.get_playlist(playlist_id)
    }

    pub fn reorder_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for (i, track_id) in track_ids.iter().enumerate() {
            conn.execute(
                "UPDATE playlist_tracks SET position = ?1, _hlc = ?2 WHERE playlist_id = ?3 AND track_id = ?4",
                rusqlite::params![i as i32, hlc, playlist_id, track_id],
            )?;
        }

        // Update playlist modified date
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE playlists SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, playlist_id],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        Ok(())
    }
}
