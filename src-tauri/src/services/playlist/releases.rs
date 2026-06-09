use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl PlaylistService {
    pub fn add_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        // Get current max position
        let max_position: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM playlist_discovery_releases WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;

        for (i, release_id) in release_ids.iter().enumerate() {
            let position = max_position + 1 + i as i32;
            conn.execute(
                "INSERT OR IGNORE INTO playlist_discovery_releases (playlist_id, release_id, position, date_added, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![playlist_id, release_id, position, now, hlc],
            )?;
        }

        // Update playlist modified date
        conn.execute(
            "UPDATE playlists SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, playlist_id],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    pub fn remove_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for release_id in &release_ids {
            let deleted = conn.execute(
                "DELETE FROM playlist_discovery_releases WHERE playlist_id = ?1 AND release_id = ?2",
                rusqlite::params![playlist_id, release_id],
            )?;
            if deleted > 0 {
                dirty::record_tombstone(
                    &conn,
                    buckets::PLAYLIST_DISCOVERY_RELEASES,
                    &dirty::junction_entity_id(playlist_id, release_id),
                    &hlc,
                )?;
            }
        }

        // Reorder remaining releases
        let remaining: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT release_id FROM playlist_discovery_releases WHERE playlist_id = ?1 ORDER BY position",
            )?;
            let result = stmt
                .query_map([playlist_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        };

        for (i, release_id) in remaining.iter().enumerate() {
            conn.execute(
                "UPDATE playlist_discovery_releases SET position = ?1, _hlc = ?2 WHERE playlist_id = ?3 AND release_id = ?4",
                rusqlite::params![i as i32, hlc, playlist_id, release_id],
            )?;
        }

        // Update playlist modified date
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE playlists SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, playlist_id],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    pub fn get_playlist_releases(&self, playlist_id: &str) -> Result<Vec<DiscoveryRelease>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label,
                dr.release_date, dr.artwork_url, dr.artwork_path,
                dr.notes, dr.parent_url, dr.source_page_url, dr.date_added, dr.date_modified
            FROM discovery_releases dr
            JOIN playlist_discovery_releases pdr ON dr.id = pdr.release_id
            WHERE pdr.playlist_id = ?1
            ORDER BY pdr.position
            "#,
        )?;

        let mut releases: Vec<DiscoveryRelease> = stmt
            .query_map([playlist_id], |row| {
                Ok(DiscoveryRelease {
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
                    is_new: false,
                    surfaced_at: None,
                    source_ids: Vec::new(),
                    tracks: Vec::new(),
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if releases.is_empty() {
            return Ok(releases);
        }

        // Batch load tracks and tags
        let release_ids: Vec<String> = releases.iter().map(|r| r.id.clone()).collect();
        let placeholders = release_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let param_refs: Vec<&dyn rusqlite::ToSql> = release_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        // Load tracks
        let mut stmt = conn.prepare(&format!(
            "SELECT id, release_id, name, position, duration_ms, video_id, is_liked FROM discovery_tracks WHERE release_id IN ({placeholders}) ORDER BY position"
        ))?;
        let all_tracks: Vec<DiscoveryTrack> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(DiscoveryTrack {
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

        // Load tags
        let mut stmt = conn.prepare(&format!(
            "SELECT drt.release_id, t.id, t.category_id, t.name, t.color, t.sort_order
             FROM tags t
             INNER JOIN discovery_release_tags drt ON t.id = drt.tag_id
             WHERE drt.release_id IN ({placeholders})
             ORDER BY t.sort_order, t.name"
        ))?;
        let all_tags: Vec<(String, Tag)> = stmt
            .query_map(param_refs.as_slice(), |row| {
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

        // Merge tracks and tags into releases
        for release in &mut releases {
            release.tracks = all_tracks
                .iter()
                .filter(|t| t.release_id == release.id)
                .cloned()
                .collect();
            release.tags = all_tags
                .iter()
                .filter(|(rid, _)| *rid == release.id)
                .map(|(_, tag)| tag.clone())
                .collect();
        }

        Ok(releases)
    }
}
