use super::*;

impl PlaylistService {
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
    pub(crate) fn count_smart_playlist_items_with_conn(
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
