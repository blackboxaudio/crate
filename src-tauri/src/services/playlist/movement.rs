use super::*;

impl PlaylistService {
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
}
