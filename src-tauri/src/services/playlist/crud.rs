use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl PlaylistService {
    pub fn get_playlists(&self, context: &str) -> Result<Vec<Playlist>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                p.id, p.name, p.parent_id, p.is_folder, p.is_smart,
                p.smart_rules, p.sort_order, p.date_created, p.date_modified,
                COALESCE(
                    CASE WHEN p.context = 'discovery'
                        THEN (SELECT COUNT(*) FROM playlist_discovery_releases WHERE playlist_id = p.id)
                        ELSE (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id)
                    END, 0
                ) as track_count,
                p.context
            FROM playlists p
            WHERE p.context = ?1
            ORDER BY p.sort_order, p.name
            "#,
        )?;

        let mut playlists = stmt
            .query_map([context], |row| {
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
                    context: row.get(10)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Compute real counts for smart playlists by evaluating their rules
        for playlist in &mut playlists {
            if playlist.is_smart {
                if let Some(ref json) = playlist.smart_rules {
                    if let Ok(rules) = serde_json::from_str::<SmartRules>(json) {
                        if let Ok(count) = self.count_smart_playlist_items_with_conn(
                            &conn,
                            &rules,
                            &playlist.context,
                        ) {
                            playlist.track_count = count;
                        }
                    }
                }
            }
        }

        Ok(playlists)
    }

    pub fn get_playlist(&self, id: &str) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut playlist = self.get_playlist_with_conn(&conn, id)?;

        // Compute real count for smart playlists
        if playlist.is_smart {
            if let Some(ref json) = playlist.smart_rules {
                if let Ok(rules) = serde_json::from_str::<SmartRules>(json) {
                    if let Ok(count) =
                        self.count_smart_playlist_items_with_conn(&conn, &rules, &playlist.context)
                    {
                        playlist.track_count = count;
                    }
                }
            }
        }

        Ok(playlist)
    }

    pub fn create_playlist(
        &self,
        name: String,
        parent_id: Option<String>,
        context: String,
    ) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

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
            context: context.clone(),
        };

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified, context, _hlc)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
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
                context,
                hlc,
            ],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        Ok(playlist)
    }

    pub fn create_folder(
        &self,
        name: String,
        parent_id: Option<String>,
        context: String,
    ) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

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
            context: context.clone(),
        };

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified, context, _hlc)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
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
                context,
                hlc,
            ],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        Ok(folder)
    }

    pub fn rename_playlist(&self, id: &str, name: String) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;

        conn.execute(
            "UPDATE playlists SET name = ?1, date_modified = ?2, _hlc = ?3 WHERE id = ?4",
            rusqlite::params![name, now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;

        drop(conn);
        self.get_playlist(id)
    }

    /// Collect all track IDs and release IDs associated with a playlist (or folder subtree).
    /// Must be called BEFORE delete_playlist since CASCADE deletes junction table entries.
    /// Returns (track_ids, release_ids) — one will be empty depending on context.
    pub fn collect_associated_item_ids(&self, id: &str) -> Result<(Vec<String>, Vec<String>)> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        // Use recursive CTE to find all playlist IDs in the subtree (handles folders)
        let mut stmt = conn.prepare(
            r#"
            WITH RECURSIVE subtree(id, context) AS (
                SELECT id, context FROM playlists WHERE id = ?1
                UNION ALL
                SELECT p.id, p.context FROM playlists p
                JOIN subtree s ON p.parent_id = s.id
            )
            SELECT id, context FROM subtree
            "#,
        )?;

        let rows: Vec<(String, String)> = stmt
            .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut track_ids = Vec::new();
        let mut release_ids = Vec::new();

        for (playlist_id, context) in &rows {
            if context == "discovery" {
                let mut rel_stmt = conn.prepare(
                    "SELECT release_id FROM playlist_discovery_releases WHERE playlist_id = ?1",
                )?;
                let ids: Vec<String> = rel_stmt
                    .query_map([playlist_id], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                release_ids.extend(ids);
            } else {
                let mut trk_stmt =
                    conn.prepare("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1")?;
                let ids: Vec<String> = trk_stmt
                    .query_map([playlist_id], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                track_ids.extend(ids);
            }
        }

        // Deduplicate
        track_ids.sort();
        track_ids.dedup();
        release_ids.sort();
        release_ids.dedup();

        Ok((track_ids, release_ids))
    }

    pub fn delete_playlist(&self, id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        // Foreign key cascade deletes child playlists + junction entries; the
        // tombstone drives the same cascade on peers.
        let hlc = dirty::next_hlc(&conn)?;
        dirty::record_tombstone(&conn, buckets::PLAYLISTS, id, &hlc)?;
        conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        dirty::mark_dirty(&conn, buckets::PLAYLISTS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;

        Ok(())
    }

    /// Get direct children of a folder
    pub fn get_children(&self, parent_id: &str) -> Result<Vec<Playlist>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                p.id, p.name, p.parent_id, p.is_folder, p.is_smart,
                p.smart_rules, p.sort_order, p.date_created, p.date_modified,
                COALESCE(
                    CASE WHEN p.context = 'discovery'
                        THEN (SELECT COUNT(*) FROM playlist_discovery_releases WHERE playlist_id = p.id)
                        ELSE (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id)
                    END, 0
                ) as track_count,
                p.context
            FROM playlists p
            WHERE p.parent_id = ?1
            ORDER BY p.sort_order, p.name
            "#,
        )?;

        let playlists = stmt
            .query_map([parent_id], |row| {
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
                    context: row.get(10)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(playlists)
    }

    /// Internal helper to get playlist with an existing connection
    pub(crate) fn get_playlist_with_conn(&self, conn: &Connection, id: &str) -> Result<Playlist> {
        conn.query_row(
            r#"
            SELECT
                p.id, p.name, p.parent_id, p.is_folder, p.is_smart,
                p.smart_rules, p.sort_order, p.date_created, p.date_modified,
                COALESCE(
                    CASE WHEN p.context = 'discovery'
                        THEN (SELECT COUNT(*) FROM playlist_discovery_releases WHERE playlist_id = p.id)
                        ELSE (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id)
                    END, 0
                ) as track_count,
                p.context
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
                    context: row.get(10)?,
                })
            },
        )
        .map_err(|e| e.into())
    }
}
