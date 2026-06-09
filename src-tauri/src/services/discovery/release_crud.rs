use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl DiscoveryService {
    pub fn create_release(&self, create: DiscoveryReleaseCreate) -> Result<DiscoveryRelease> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let normalized_url = normalize_url(&create.url);
        let source_type = create
            .source_type
            .unwrap_or_else(|| detect_source_type(&normalized_url));

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, artist, title, label, release_date, artwork_url, notes, parent_url, source_page_url, date_added, date_modified, _hlc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                id,
                normalized_url,
                source_type,
                create.artist,
                create.title,
                create.label,
                create.release_date,
                create.artwork_url,
                create.notes,
                create.parent_url,
                create.source_page_url,
                now,
                now,
                hlc,
            ],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;

        // Insert tracks if provided
        let mut tracks = Vec::new();
        if let Some(track_creates) = create.tracks {
            for tc in track_creates {
                let track_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms, video_id, _hlc) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![track_id, id, tc.name, tc.position, tc.duration_ms, tc.video_id, hlc],
                )?;
                tracks.push(DiscoveryTrack {
                    id: track_id,
                    release_id: id.clone(),
                    name: tc.name,
                    position: tc.position,
                    duration_ms: tc.duration_ms,
                    video_id: tc.video_id,
                    is_liked: false,
                });
            }
        }
        dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACKS)?;

        Ok(DiscoveryRelease {
            id,
            url: normalized_url,
            source_type,
            artist: create.artist,
            title: create.title,
            label: create.label,
            release_date: create.release_date,
            artwork_url: create.artwork_url,
            artwork_path: None,
            notes: create.notes,
            parent_url: create.parent_url,
            source_page_url: create.source_page_url,
            date_added: now.clone(),
            date_modified: now,
            is_new: false,
            surfaced_at: None,
            source_ids: Vec::new(),
            tracks,
            tags: Vec::new(),
        })
    }

    pub fn get_release(&self, id: &str) -> Result<DiscoveryRelease> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let mut release = conn.query_row(
            "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, artwork_path, notes, parent_url, source_page_url, date_added, date_modified, is_new, surfaced_at
             FROM discovery_releases WHERE id = ?1",
            [id],
            |row| {
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
                    is_new: row.get::<_, i32>(14).map(|v| v != 0)?,
                    surfaced_at: row.get(15)?,
                    source_ids: Vec::new(),
                    tracks: Vec::new(),
                    tags: Vec::new(),
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CrateError::Discovery(format!("Release not found: {id}"))
            }
            _ => CrateError::Database(e),
        })?;

        // Load tracks
        let mut stmt = conn.prepare(
            "SELECT id, release_id, name, position, duration_ms, video_id, is_liked FROM discovery_tracks WHERE release_id = ?1 ORDER BY position",
        )?;
        release.tracks = stmt
            .query_map([id], |row| {
                Ok(DiscoveryTrack {
                    id: row.get(0)?,
                    release_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    duration_ms: row.get(4)?,
                    video_id: row.get(5)?,
                    is_liked: row.get::<_, i32>(6).map(|v| v != 0)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Load tags
        let mut stmt = conn.prepare(
            "SELECT t.id, t.category_id, t.name, t.color, t.sort_order
             FROM tags t
             INNER JOIN discovery_release_tags drt ON t.id = drt.tag_id
             WHERE drt.release_id = ?1
             ORDER BY t.sort_order, t.name",
        )?;
        release.tags = stmt
            .query_map([id], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Load provenance: ids of the followed sources that surfaced this release.
        let mut stmt =
            conn.prepare("SELECT source_id FROM discovery_release_sources WHERE release_id = ?1")?;
        release.source_ids = stmt
            .query_map([id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(release)
    }

    pub fn get_releases(&self, filter: Option<DiscoveryFilter>) -> Result<Vec<DiscoveryRelease>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let filter = filter.unwrap_or_default();

        // Build query with optional filters
        let mut sql = String::from(
            "SELECT DISTINCT dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label, dr.release_date, dr.artwork_url, dr.artwork_path, dr.notes, dr.parent_url, dr.source_page_url, dr.date_added, dr.date_modified, dr.is_new, dr.surfaced_at
             FROM discovery_releases dr",
        );

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // Tag filter join
        if let Some(ref tag_ids) = filter.tag_ids {
            if !tag_ids.is_empty() {
                let mode = filter.tag_filter_mode.as_deref().unwrap_or("or");

                if mode == "and" {
                    // AND mode: release must have ALL specified tags
                    sql.push_str(&format!(
                        " INNER JOIN discovery_release_tags drt ON dr.id = drt.release_id AND drt.tag_id IN ({})",
                        tag_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
                    ));
                    for tag_id in tag_ids {
                        params.push(Box::new(tag_id.clone()));
                    }
                    conditions.push(format!(
                        "1=1 GROUP BY dr.id HAVING COUNT(DISTINCT drt.tag_id) = {}",
                        tag_ids.len()
                    ));
                } else {
                    // OR mode: release must have ANY of the specified tags
                    sql.push_str(&format!(
                        " INNER JOIN discovery_release_tags drt ON dr.id = drt.release_id AND drt.tag_id IN ({})",
                        tag_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
                    ));
                    for tag_id in tag_ids {
                        params.push(Box::new(tag_id.clone()));
                    }
                }
            }
        }

        // Search filter
        if let Some(ref search) = filter.search {
            let search_pattern = format!("%{search}%");
            conditions.push(
                "(dr.artist LIKE ? OR dr.title LIKE ? OR dr.label LIKE ? OR dr.notes LIKE ? OR EXISTS (SELECT 1 FROM discovery_tracks dt WHERE dt.release_id = dr.id AND dt.name LIKE ?))"
                    .to_string(),
            );
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern));
        }

        if !conditions.is_empty() {
            // Check if we have a GROUP BY in conditions (from AND tag filter)
            let has_group_by = conditions.iter().any(|c| c.contains("GROUP BY"));
            if has_group_by {
                // The GROUP BY condition handles WHERE internally
                let group_condition = conditions
                    .iter()
                    .find(|c| c.contains("GROUP BY"))
                    .unwrap()
                    .clone();
                let other_conditions: Vec<&str> = conditions
                    .iter()
                    .filter(|c| !c.contains("GROUP BY"))
                    .map(|c| c.as_str())
                    .collect();

                if !other_conditions.is_empty() {
                    sql.push_str(&format!(" WHERE {}", other_conditions.join(" AND ")));
                }
                sql.push_str(&format!(" {}", group_condition.replace("1=1 ", "")));
            } else {
                sql.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
            }
        }

        sql.push_str(" ORDER BY dr.date_added DESC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

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
                    source_page_url: row.get(11)?,
                    date_added: row.get(12)?,
                    date_modified: row.get(13)?,
                    is_new: row.get::<_, i32>(14).map(|v| v != 0)?,
                    surfaced_at: row.get(15)?,
                    source_ids: Vec::new(),
                    tracks: Vec::new(),
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if releases.is_empty() {
            return Ok(releases);
        }

        // Batch load tracks for all releases
        let release_ids: Vec<String> = releases.iter().map(|r| r.id.clone()).collect();
        let placeholders = release_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let mut stmt = conn.prepare(&format!(
            "SELECT id, release_id, name, position, duration_ms, video_id, is_liked FROM discovery_tracks WHERE release_id IN ({placeholders}) ORDER BY position"
        ))?;
        let track_params: Vec<&dyn rusqlite::types::ToSql> = release_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let all_tracks: Vec<DiscoveryTrack> = stmt
            .query_map(track_params.as_slice(), |row| {
                Ok(DiscoveryTrack {
                    id: row.get(0)?,
                    release_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    duration_ms: row.get(4)?,
                    video_id: row.get(5)?,
                    is_liked: row.get::<_, i32>(6).map(|v| v != 0)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Batch load tags for all releases
        let mut stmt = conn.prepare(&format!(
            "SELECT drt.release_id, t.id, t.category_id, t.name, t.color, t.sort_order
             FROM tags t
             INNER JOIN discovery_release_tags drt ON t.id = drt.tag_id
             WHERE drt.release_id IN ({placeholders})
             ORDER BY t.sort_order, t.name"
        ))?;

        let all_tags: Vec<(String, Tag)> = stmt
            .query_map(track_params.as_slice(), |row| {
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

        // Batch load provenance (followed sources that surfaced each release).
        let mut stmt = conn.prepare(&format!(
            "SELECT release_id, source_id FROM discovery_release_sources WHERE release_id IN ({placeholders})"
        ))?;
        let all_sources: Vec<(String, String)> = stmt
            .query_map(track_params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Merge tracks, tags, and provenance into releases
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
            release.source_ids = all_sources
                .iter()
                .filter(|(rid, _)| *rid == release.id)
                .map(|(_, sid)| sid.clone())
                .collect();
        }

        Ok(releases)
    }

    pub fn update_release(
        &self,
        id: &str,
        update: DiscoveryReleaseUpdate,
    ) -> Result<DiscoveryRelease> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut set_clauses = vec!["date_modified = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        // Empty string from the frontend means "clear this field" → bind SQL NULL
        let str_or_null = |s: &str| -> Option<String> {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        };

        if let Some(ref artist) = update.artist {
            set_clauses.push("artist = ?".to_string());
            params.push(Box::new(str_or_null(artist)));
        }
        if let Some(ref title) = update.title {
            set_clauses.push("title = ?".to_string());
            params.push(Box::new(str_or_null(title)));
        }
        if let Some(ref label) = update.label {
            set_clauses.push("label = ?".to_string());
            params.push(Box::new(str_or_null(label)));
        }
        if let Some(ref release_date) = update.release_date {
            set_clauses.push("release_date = ?".to_string());
            params.push(Box::new(str_or_null(release_date)));
        }
        if let Some(ref artwork_url) = update.artwork_url {
            set_clauses.push("artwork_url = ?".to_string());
            params.push(Box::new(str_or_null(artwork_url)));
        }
        if let Some(ref artwork_path) = update.artwork_path {
            set_clauses.push("artwork_path = ?".to_string());
            params.push(Box::new(str_or_null(artwork_path)));
        }
        if let Some(ref notes) = update.notes {
            set_clauses.push("notes = ?".to_string());
            params.push(Box::new(str_or_null(notes)));
        }

        let hlc = dirty::next_hlc(&conn)?;
        set_clauses.push("_hlc = ?".to_string());
        params.push(Box::new(hlc));

        params.push(Box::new(id.to_string()));

        let sql = format!(
            "UPDATE discovery_releases SET {} WHERE id = ?",
            set_clauses.join(", ")
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;

        drop(conn);
        self.get_release(id)
    }

    /// Backfill `source_page_url` for the release with this URL, only when it's unset.
    /// Used when re-scanning/re-importing a page so releases imported before the page
    /// was recorded (e.g. existing rows skipped on the UNIQUE(url) conflict) get linked
    /// to the followed page. Bumps `_hlc` so the change syncs.
    pub fn set_source_page_url_if_absent(
        &self,
        release_url: &str,
        page_url: &str,
    ) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let normalized = normalize_url(release_url);
        let hlc = dirty::next_hlc(&conn)?;
        let changed = conn.execute(
            "UPDATE discovery_releases SET source_page_url = ?1, _hlc = ?2 \
             WHERE url = ?3 AND (source_page_url IS NULL OR source_page_url = '')",
            rusqlite::params![page_url, hlc, normalized],
        )?;
        if changed > 0 {
            dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
        }
        Ok(changed)
    }

    pub fn set_release_artwork(
        &self,
        id: &str,
        file_path: &std::path::Path,
    ) -> Result<DiscoveryRelease> {
        let relative_path = self
            .artwork_service
            .save_from_file(file_path, id)
            .ok_or_else(|| CrateError::Artwork("Failed to save artwork".to_string()))?;

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE discovery_releases SET artwork_path = ?1, date_modified = ?2, _hlc = ?3 WHERE id = ?4",
            rusqlite::params![relative_path, now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;

        drop(conn);
        self.get_release(id)
    }

    pub fn delete_release_artwork(&self, id: &str) -> Result<DiscoveryRelease> {
        self.artwork_service.delete(id);

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE discovery_releases SET artwork_path = NULL, date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;

        drop(conn);
        self.get_release(id)
    }

    pub fn delete_release(&self, id: &str) -> Result<()> {
        // Clean up cached audio files before the SQL DELETE
        if let Err(e) = self.delete_cached_audio_files(id) {
            log::warn!("Failed to clean up cached audio for release {id}: {e}");
        }

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        // Delete = dismiss: tombstone this release's followed-source record so the watch
        // loop never re-adds it. Runs before the DELETE so the URL is still resolvable;
        // best-effort (a hiccup must never block the delete).
        let _ = conn.execute(
            "UPDATE followed_source_releases SET status = 'dismissed' \
             WHERE seen_url = (SELECT url FROM discovery_releases WHERE id = ?1)",
            [id],
        );

        let hlc = dirty::next_hlc(&conn)?;
        dirty::record_tombstone(&conn, buckets::DISCOVERY_RELEASES, id, &hlc)?;
        conn.execute("DELETE FROM discovery_releases WHERE id = ?1", [id])?;
        // Cascade removes this release's tracks + tag/playlist links.
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
        Ok(())
    }

    pub fn get_all_release_urls(&self) -> Result<std::collections::HashSet<String>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare("SELECT url FROM discovery_releases")?;
        let urls = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<std::collections::HashSet<String>, _>>()?;

        Ok(urls)
    }

    pub fn toggle_track_liked(&self, track_id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE discovery_tracks SET is_liked = CASE WHEN is_liked = 0 THEN 1 ELSE 0 END, _hlc = ?2 WHERE id = ?1",
            rusqlite::params![track_id, hlc],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACKS)?;

        let is_liked: bool = conn
            .query_row(
                "SELECT is_liked FROM discovery_tracks WHERE id = ?1",
                [track_id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CrateError::Discovery(format!("Track not found: {track_id}"))
                }
                _ => CrateError::Database(e),
            })?;

        Ok(is_liked)
    }

    pub fn delete_releases(&self, ids: Vec<String>) -> Result<()> {
        // Clean up cached audio files for all releases
        for id in &ids {
            if let Err(e) = self.delete_cached_audio_files(id) {
                log::warn!("Failed to clean up cached audio for release {id}: {e}");
            }
        }

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!("DELETE FROM discovery_releases WHERE id IN ({placeholders})");
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();

        // Delete = dismiss: tombstone these releases' followed-source records before the
        // DELETE so the watch loop never re-adds them. Best-effort.
        let _ = conn.execute(
            &format!(
                "UPDATE followed_source_releases SET status = 'dismissed' \
                 WHERE seen_url IN (SELECT url FROM discovery_releases WHERE id IN ({placeholders}))"
            ),
            params.as_slice(),
        );

        let hlc = dirty::next_hlc(&conn)?;
        for id in &ids {
            dirty::record_tombstone(&conn, buckets::DISCOVERY_RELEASES, id, &hlc)?;
        }

        conn.execute(&sql, params.as_slice())?;

        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;

        Ok(())
    }
}
