use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{MoveConflict, MovePlaylistResult, Playlist, Tag, Track};

pub struct PlaylistService {
    conn: Arc<Mutex<Connection>>,
}

impl PlaylistService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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
                    COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id), 0) as track_count
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
                COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id), 0) as track_count
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

    pub fn add_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
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

        Ok(())
    }

    pub fn remove_tracks(&self, playlist_id: &str, track_ids: Vec<String>) -> Result<()> {
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
}
