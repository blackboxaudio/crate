use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryRelease, DiscoveryTrack, MoveConflict, MovePlaylistResult, Playlist, SmartRules, Tag,
    Track,
};
use crate::services::smart_rules;

pub struct PlaylistService {
    conn: Arc<Mutex<Connection>>,
}

impl PlaylistService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_playlists(&self, context: &str) -> Result<Vec<Playlist>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

    pub fn create_playlist(
        &self,
        name: String,
        parent_id: Option<String>,
        context: String,
    ) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified, context)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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
            ],
        )?;

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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified, context)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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
            ],
        )?;

        Ok(folder)
    }

    pub fn rename_playlist(&self, id: &str, name: String) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE playlists SET name = ?1, date_modified = ?2 WHERE id = ?3",
            rusqlite::params![name, now, id],
        )?;

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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // Foreign key cascade will delete playlist_tracks entries
        conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;

        Ok(())
    }

    /// Find a playlist/folder with the same name in the target location (excluding the item being moved)
    pub fn find_conflict(&self, id: &str, parent_id: Option<&str>) -> Result<Option<Playlist>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let moving = self.get_playlist_with_conn(&conn, id)?;

        let conflict: Option<Playlist> = conn
            .query_row(
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
                WHERE p.parent_id IS ?1 AND p.name = ?2 AND p.id != ?3
                "#,
                rusqlite::params![parent_id, moving.name, id],
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
            .ok();

        Ok(conflict)
    }

    /// Get direct children of a folder
    pub fn get_children(&self, parent_id: &str) -> Result<Vec<Playlist>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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
    fn get_playlist_with_conn(&self, conn: &Connection, id: &str) -> Result<Playlist> {
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

    /// Internal helper to perform the actual move operation
    fn do_move(&self, id: &str, parent_id: Option<String>) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        // Get next sort order in the target location
        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM playlists WHERE parent_id IS ?1",
                rusqlite::params![parent_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        conn.execute(
            "UPDATE playlists SET parent_id = ?1, sort_order = ?2, date_modified = ?3 WHERE id = ?4",
            rusqlite::params![parent_id, max_order + 1, now, id],
        )?;

        drop(conn);
        self.get_playlist(id)
    }

    /// Merge source folder's contents into target folder
    fn merge_folders(&self, source_id: &str, target_id: &str) -> Result<MovePlaylistResult> {
        // Get all direct children of source folder
        let children = self.get_children(source_id)?;
        let mut nested_conflicts = Vec::new();

        for child in children {
            // Check if this child conflicts with something in target
            if let Some(conflict) = self.find_conflict(&child.id, Some(target_id))? {
                nested_conflicts.push(MoveConflict {
                    moving_item: child,
                    existing_item: conflict,
                });
            } else {
                // No conflict, move child directly to target
                self.do_move(&child.id, Some(target_id.to_string()))?;
            }
        }

        // If all children moved without conflicts, delete the now-empty source folder
        if nested_conflicts.is_empty() {
            self.delete_playlist(source_id)?;
        }

        let target = self.get_playlist(target_id)?;

        Ok(MovePlaylistResult {
            playlist: target,
            nested_conflicts,
        })
    }

    /// Merge source playlist's tracks into target playlist
    fn merge_playlists(&self, source_id: &str, target_id: &str) -> Result<MovePlaylistResult> {
        // Get tracks from source playlist
        let source_tracks = self.get_playlist_tracks(source_id)?;
        let track_ids: Vec<String> = source_tracks.iter().map(|t| t.id.clone()).collect();

        // Add tracks to target playlist (INSERT OR IGNORE handles duplicates)
        if !track_ids.is_empty() {
            self.add_tracks(target_id, track_ids)?;
        }

        // Delete source playlist
        self.delete_playlist(source_id)?;

        // Return target playlist
        let target = self.get_playlist(target_id)?;

        Ok(MovePlaylistResult {
            playlist: target,
            nested_conflicts: vec![],
        })
    }

    /// Move playlist with optional conflict resolution
    pub fn move_playlist(
        &self,
        id: &str,
        parent_id: Option<String>,
        resolution: Option<&str>,
    ) -> Result<MovePlaylistResult> {
        // Check for conflict
        let conflict = self.find_conflict(id, parent_id.as_deref())?;

        if let Some(existing) = conflict {
            match resolution {
                Some("overwrite") => {
                    // Verify both are same type (folder-folder or playlist-playlist)
                    let moving = self.get_playlist(id)?;
                    if moving.is_folder != existing.is_folder {
                        return Err(CrateError::InvalidOperation(
                            "Cannot overwrite: items must be the same type".to_string(),
                        ));
                    }
                    // Delete existing, then move
                    self.delete_playlist(&existing.id)?;
                    let playlist = self.do_move(id, parent_id)?;
                    Ok(MovePlaylistResult {
                        playlist,
                        nested_conflicts: vec![],
                    })
                }
                Some("merge") => {
                    // Verify both are same type
                    let moving = self.get_playlist(id)?;
                    if moving.is_folder != existing.is_folder {
                        return Err(CrateError::InvalidOperation(
                            "Cannot merge: items must be the same type".to_string(),
                        ));
                    }

                    if moving.is_folder {
                        self.merge_folders(id, &existing.id)
                    } else {
                        self.merge_playlists(id, &existing.id)
                    }
                }
                None | Some(_) => {
                    // No resolution provided but conflict exists - return error
                    Err(CrateError::InvalidOperation(format!(
                        "Name conflict: an item named '{}' already exists at the target location",
                        existing.name
                    )))
                }
            }
        } else {
            // No conflict, proceed with normal move
            let playlist = self.do_move(id, parent_id)?;
            Ok(MovePlaylistResult {
                playlist,
                nested_conflicts: vec![],
            })
        }
    }

    pub fn get_playlist(&self, id: &str) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

    pub fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                t.id, t.file_path, t.file_hash,
                t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                t.analysis_source, t.waveform_data,
                t.rating, t.play_count,
                t.date_added, t.date_modified, t.last_played,
                t.rekordbox_id, t.artwork_path, t.artwork_source, t.color
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
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Fetch tags
        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, tracks)?;
        Ok(tracks_with_tags)
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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

        // Drop the lock before calling get_playlist which acquires its own lock
        drop(conn);

        // Return the updated playlist with accurate track count
        self.get_playlist(playlist_id)
    }

    pub fn remove_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for track_id in &track_ids {
            conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                rusqlite::params![playlist_id, track_id],
            )?;
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

        // Drop the lock before calling get_playlist which acquires its own lock
        drop(conn);

        // Return the updated playlist with accurate track count
        self.get_playlist(playlist_id)
    }

    pub fn reorder_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

    // =========================================================================
    // Discovery Release Operations
    // =========================================================================

    pub fn add_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // Get current max position
        let max_position: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM playlist_discovery_releases WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let now = chrono::Utc::now().to_rfc3339();

        for (i, release_id) in release_ids.iter().enumerate() {
            let position = max_position + 1 + i as i32;
            conn.execute(
                "INSERT OR IGNORE INTO playlist_discovery_releases (playlist_id, release_id, position, date_added) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![playlist_id, release_id, position, now],
            )?;
        }

        // Update playlist modified date
        conn.execute(
            "UPDATE playlists SET date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, playlist_id],
        )?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    pub fn remove_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<Playlist> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for release_id in &release_ids {
            conn.execute(
                "DELETE FROM playlist_discovery_releases WHERE playlist_id = ?1 AND release_id = ?2",
                rusqlite::params![playlist_id, release_id],
            )?;
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
                "UPDATE playlist_discovery_releases SET position = ?1 WHERE playlist_id = ?2 AND release_id = ?3",
                rusqlite::params![i as i32, playlist_id, release_id],
            )?;
        }

        // Update playlist modified date
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE playlists SET date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, playlist_id],
        )?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    pub fn get_playlist_releases(&self, playlist_id: &str) -> Result<Vec<DiscoveryRelease>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label,
                dr.release_date, dr.artwork_url, dr.artwork_path,
                dr.notes, dr.parent_url, dr.date_added, dr.date_modified
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
                    date_added: row.get(11)?,
                    date_modified: row.get(12)?,
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
            "SELECT id, release_id, name, position, duration_ms, video_id FROM discovery_tracks WHERE release_id IN ({placeholders}) ORDER BY position"
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

    // =========================================================================
    // Smart Playlist Operations
    // =========================================================================

    pub fn create_smart_playlist(
        &self,
        name: String,
        parent_id: Option<String>,
        context: String,
        smart_rules_json: String,
    ) -> Result<Playlist> {
        let rules: SmartRules = serde_json::from_str(&smart_rules_json)
            .map_err(|e| CrateError::InvalidOperation(format!("Invalid smart rules JSON: {e}")))?;

        smart_rules::validate_smart_rules(&rules, &context)?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM playlists WHERE parent_id IS ?1",
                rusqlite::params![parent_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        conn.execute(
            r#"
            INSERT INTO playlists (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, date_created, date_modified, context)
            VALUES (?1, ?2, ?3, 0, 1, ?4, ?5, ?6, ?7, ?8)
            "#,
            rusqlite::params![
                id,
                name,
                parent_id,
                smart_rules_json,
                max_order + 1,
                now,
                now,
                context,
            ],
        )?;

        // Compute the track count by evaluating rules
        let track_count = self.count_smart_playlist_items_with_conn(&conn, &rules, &context)?;

        Ok(Playlist {
            id,
            name,
            parent_id,
            is_folder: false,
            is_smart: true,
            smart_rules: Some(smart_rules_json),
            sort_order: max_order + 1,
            date_created: now.clone(),
            date_modified: now,
            track_count,
            context,
        })
    }

    pub fn update_smart_rules(&self, id: &str, smart_rules_json: String) -> Result<Playlist> {
        // Fetch the playlist to know its context
        let playlist = self.get_playlist(id)?;
        if !playlist.is_smart {
            return Err(CrateError::InvalidOperation(
                "Cannot update smart rules on a non-smart playlist".to_string(),
            ));
        }

        let rules: SmartRules = serde_json::from_str(&smart_rules_json)
            .map_err(|e| CrateError::InvalidOperation(format!("Invalid smart rules JSON: {e}")))?;

        smart_rules::validate_smart_rules(&rules, &playlist.context)?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE playlists SET smart_rules = ?1, date_modified = ?2 WHERE id = ?3",
            rusqlite::params![smart_rules_json, now, id],
        )?;

        drop(conn);
        self.get_playlist(id)
    }

    pub fn get_smart_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let playlist = self.get_playlist(playlist_id)?;
        if !playlist.is_smart {
            return Err(CrateError::InvalidOperation(
                "Playlist is not a smart playlist".to_string(),
            ));
        }

        let rules: SmartRules = match &playlist.smart_rules {
            Some(json) => serde_json::from_str(json).map_err(|e| {
                CrateError::InvalidOperation(format!("Invalid smart rules JSON: {e}"))
            })?,
            None => return Ok(Vec::new()),
        };

        let (where_clause, params) = smart_rules::build_smart_query_library(&rules)?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let sql = format!(
            r#"
            SELECT
                t.id, t.file_path, t.file_hash,
                t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                t.analysis_source, t.waveform_data,
                t.rating, t.play_count,
                t.date_added, t.date_modified, t.last_played,
                t.rekordbox_id, t.artwork_path, t.artwork_source, t.color
            FROM tracks t
            WHERE {where_clause}
            "#,
        );

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let mut stmt = conn.prepare(&sql)?;
        let tracks = stmt
            .query_map(param_refs.as_slice(), |row| {
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
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, tracks)?;
        Ok(tracks_with_tags)
    }

    pub fn get_smart_playlist_releases(&self, playlist_id: &str) -> Result<Vec<DiscoveryRelease>> {
        let playlist = self.get_playlist(playlist_id)?;
        if !playlist.is_smart {
            return Err(CrateError::InvalidOperation(
                "Playlist is not a smart playlist".to_string(),
            ));
        }

        let rules: SmartRules = match &playlist.smart_rules {
            Some(json) => serde_json::from_str(json).map_err(|e| {
                CrateError::InvalidOperation(format!("Invalid smart rules JSON: {e}"))
            })?,
            None => return Ok(Vec::new()),
        };

        let (where_clause, params) = smart_rules::build_smart_query_discovery(&rules)?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let sql = format!(
            r#"
            SELECT
                dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label,
                dr.release_date, dr.artwork_url, dr.artwork_path,
                dr.notes, dr.parent_url, dr.date_added, dr.date_modified
            FROM discovery_releases dr
            WHERE {where_clause}
            "#,
        );

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let mut stmt = conn.prepare(&sql)?;
        let mut releases: Vec<DiscoveryRelease> = stmt
            .query_map(param_refs.as_slice(), |row| {
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
                    date_added: row.get(11)?,
                    date_modified: row.get(12)?,
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

        let mut stmt = conn.prepare(&format!(
            "SELECT id, release_id, name, position, duration_ms, video_id FROM discovery_tracks WHERE release_id IN ({placeholders}) ORDER BY position"
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
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

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

    pub fn preview_smart_rules_count(&self, smart_rules_json: &str, context: &str) -> Result<i32> {
        let rules: SmartRules = serde_json::from_str(smart_rules_json)
            .map_err(|e| CrateError::InvalidOperation(format!("Invalid smart rules JSON: {e}")))?;

        smart_rules::validate_smart_rules(&rules, context)?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        self.count_smart_playlist_items_with_conn(&conn, &rules, context)
    }

    /// Count matching items for smart rules using an existing connection.
    fn count_smart_playlist_items_with_conn(
        &self,
        conn: &Connection,
        rules: &SmartRules,
        context: &str,
    ) -> Result<i32> {
        let (where_clause, params) = if context == "discovery" {
            smart_rules::build_smart_query_discovery(rules)?
        } else {
            smart_rules::build_smart_query_library(rules)?
        };

        // Strip ORDER BY and LIMIT for count query
        let where_only = if let Some(idx) = where_clause.find(" ORDER BY") {
            &where_clause[..idx]
        } else {
            &where_clause
        };

        let table = if context == "discovery" {
            "discovery_releases dr"
        } else {
            "tracks t"
        };

        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {where_only}");

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let count: i32 = conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?;

        Ok(count)
    }
}
