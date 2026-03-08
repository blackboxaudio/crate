use super::*;

impl ExportService {
    /// Collect all tracks from selected playlists, recursively including folder contents
    pub(super) fn collect_tracks_for_export(
        &self,
        playlist_ids: &[String],
    ) -> Result<Vec<(Playlist, Vec<Track>)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut result: Vec<(Playlist, Vec<Track>)> = Vec::new();

        for playlist_id in playlist_ids {
            // Get the playlist
            let playlist: Playlist = conn.query_row(
                r#"
                SELECT id, name, parent_id, is_folder, is_smart, smart_rules,
                       sort_order, date_created, date_modified,
                       COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = id), 0)
                FROM playlists WHERE id = ?1
                "#,
                [playlist_id],
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
                        context: "library".to_string(),
                    })
                },
            )?;

            if playlist.is_folder {
                // Recursively get all child playlists
                let child_ids = self.get_all_descendant_playlist_ids(&conn, playlist_id)?;
                for child_id in child_ids {
                    let child_playlist: Playlist = conn.query_row(
                        r#"
                        SELECT id, name, parent_id, is_folder, is_smart, smart_rules,
                               sort_order, date_created, date_modified,
                               COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = id), 0)
                        FROM playlists WHERE id = ?1
                        "#,
                        [&child_id],
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
                                context: "library".to_string(),
                            })
                        },
                    )?;

                    if !child_playlist.is_folder {
                        let tracks = if child_playlist.is_smart {
                            self.get_smart_playlist_tracks_for_export(&conn, &child_playlist)?
                        } else {
                            self.get_playlist_tracks(&conn, &child_id)?
                        };
                        result.push((child_playlist, tracks));
                    }
                }
                // Also add the folder itself (for tree structure in PDB)
                result.push((playlist, vec![]));
            } else if playlist.is_smart {
                // Smart playlist - evaluate rules to get tracks
                let tracks = self.get_smart_playlist_tracks_for_export(&conn, &playlist)?;
                result.push((playlist, tracks));
            } else {
                // Regular playlist - get its tracks
                let tracks = self.get_playlist_tracks(&conn, playlist_id)?;
                result.push((playlist, tracks));
            }
        }

        Ok(result)
    }

    /// Get all descendant playlist IDs (recursive)
    pub(super) fn get_all_descendant_playlist_ids(
        &self,
        conn: &Connection,
        parent_id: &str,
    ) -> Result<Vec<String>> {
        get_all_descendant_playlist_ids_impl(conn, parent_id)
    }

    /// Get tracks for a playlist in order
    pub(super) fn get_playlist_tracks(
        &self,
        conn: &Connection,
        playlist_id: &str,
    ) -> Result<Vec<Track>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT t.id, t.file_path, t.file_hash,
                   t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                   t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                   t.analysis_source, t.waveform_data, t.rating, t.play_count,
                   t.date_added, t.date_modified, t.last_played, t.rekordbox_id,
                   t.artwork_path, t.artwork_source, t.color
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
                    tags: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

    /// Get tracks for a smart playlist by evaluating its rules
    pub(super) fn get_smart_playlist_tracks_for_export(
        &self,
        conn: &Connection,
        playlist: &Playlist,
    ) -> Result<Vec<Track>> {
        let rules: crate::models::SmartRules = match &playlist.smart_rules {
            Some(json) => serde_json::from_str(json).map_err(|e| {
                CrateError::InvalidOperation(format!("Invalid smart rules JSON: {e}"))
            })?,
            None => return Ok(Vec::new()),
        };

        let (where_clause, params) =
            crate::services::smart_rules::build_smart_query_library(&rules)?;

        let sql = format!(
            r#"
            SELECT t.id, t.file_path, t.file_hash,
                   t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                   t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                   t.analysis_source, t.waveform_data, t.rating, t.play_count,
                   t.date_added, t.date_modified, t.last_played, t.rekordbox_id,
                   t.artwork_path, t.artwork_source, t.color
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
                    tags: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tracks)
    }
}
