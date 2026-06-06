use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl DiscoveryService {
    /// Find existing releases that may overlap with the given metadata.
    /// Checks by exact URL, parent_url match, and artist+title match.
    pub fn find_matching_releases(
        &self,
        url: Option<&str>,
        artist: Option<&str>,
        title: Option<&str>,
        parent_url: Option<&str>,
    ) -> Result<Vec<DiscoveryRelease>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut matched_ids: Vec<String> = Vec::new();

        // Check 0: Exact URL match
        if let Some(u) = url {
            let normalized = normalize_url(u);
            let mut stmt = conn.prepare("SELECT id FROM discovery_releases WHERE url = ?1")?;
            let ids: Vec<String> = stmt
                .query_map([&normalized], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            matched_ids.extend(ids);
        }

        // Check 1: If parent_url provided, find releases whose url matches the parent_url
        if let Some(p_url) = parent_url {
            let normalized = normalize_url(p_url);
            let mut stmt = conn.prepare("SELECT id FROM discovery_releases WHERE url = ?1")?;
            let ids: Vec<String> = stmt
                .query_map([&normalized], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            matched_ids.extend(ids);
        }

        // Check 2: If parent_url provided, find releases with the same parent_url
        if let Some(p_url) = parent_url {
            let normalized = normalize_url(p_url);
            let mut stmt =
                conn.prepare("SELECT id FROM discovery_releases WHERE parent_url = ?1")?;
            let ids: Vec<String> = stmt
                .query_map([&normalized], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            matched_ids.extend(ids);
        }

        // Check 3: If artist+title provided, find case-insensitive matches
        if let (Some(a), Some(t)) = (artist, title) {
            let mut stmt = conn.prepare(
                "SELECT id FROM discovery_releases WHERE LOWER(artist) = LOWER(?1) AND LOWER(title) = LOWER(?2)",
            )?;
            let ids: Vec<String> = stmt
                .query_map(rusqlite::params![a, t], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            matched_ids.extend(ids);
        }

        // Deduplicate IDs
        matched_ids.sort();
        matched_ids.dedup();

        drop(conn);

        // Fetch full release objects
        matched_ids.iter().map(|id| self.get_release(id)).collect()
    }

    /// Add tracks to an existing release, deduplicating by name (case-insensitive).
    pub fn add_tracks_to_release(
        &self,
        release_id: &str,
        tracks: Vec<DiscoveryTrackCreate>,
    ) -> Result<DiscoveryRelease> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        {
            // Get existing track names for deduplication
            let mut stmt = conn.prepare(
                "SELECT name, MAX(position) FROM discovery_tracks WHERE release_id = ?1 GROUP BY LOWER(name)",
            )?;

            let existing: Vec<(String, i32)> = stmt
                .query_map([release_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let existing_names: Vec<String> = existing
                .iter()
                .map(|(name, _)| name.to_lowercase())
                .collect();
            let max_position = existing.iter().map(|(_, pos)| *pos).max().unwrap_or(0);

            let now = chrono::Utc::now().to_rfc3339();
            let hlc = dirty::next_hlc(&conn)?;
            let mut next_position = max_position + 1;

            for track in &tracks {
                if existing_names.contains(&track.name.to_lowercase()) {
                    continue;
                }
                let track_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms, video_id, _hlc) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![track_id, release_id, track.name, next_position, track.duration_ms, track.video_id, hlc],
                )?;
                next_position += 1;
            }

            // Update date_modified
            conn.execute(
                "UPDATE discovery_releases SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
                rusqlite::params![now, hlc, release_id],
            )?;
            dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACKS)?;
            dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
        }

        drop(conn);
        self.get_release(release_id)
    }

    /// Merge source releases into a target release.
    /// Copies tracks (deduped by name), unions tags, moves playlist memberships,
    /// concatenates notes, then deletes source releases.
    pub fn merge_releases(
        &self,
        target_id: &str,
        source_ids: Vec<String>,
    ) -> Result<DiscoveryRelease> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        {
            let now = chrono::Utc::now().to_rfc3339();
            let hlc = dirty::next_hlc(&conn)?;

            // Get existing target track names for dedup
            let mut stmt = conn.prepare(
                "SELECT LOWER(name), MAX(position) FROM discovery_tracks WHERE release_id = ?1 GROUP BY LOWER(name)",
            )?;
            let existing: Vec<(String, i32)> = stmt
                .query_map([target_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let mut existing_names: Vec<String> = existing.iter().map(|(n, _)| n.clone()).collect();
            let mut next_position = existing.iter().map(|(_, p)| *p).max().unwrap_or(0) + 1;

            // Get target notes
            let target_notes: Option<String> = conn.query_row(
                "SELECT notes FROM discovery_releases WHERE id = ?1",
                [target_id],
                |row| row.get(0),
            )?;

            let mut all_notes: Vec<String> = Vec::new();
            if let Some(ref notes) = target_notes {
                if !notes.is_empty() {
                    all_notes.push(notes.clone());
                }
            }

            for source_id in &source_ids {
                // Copy tracks from source, deduplicating
                let mut stmt = conn.prepare(
                    "SELECT name, position, duration_ms, video_id FROM discovery_tracks WHERE release_id = ?1 ORDER BY position",
                )?;
                let source_tracks: Vec<(String, i32, Option<i64>, Option<String>)> = stmt
                    .query_map([source_id.as_str()], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                for (name, _, duration_ms, video_id) in &source_tracks {
                    if existing_names.contains(&name.to_lowercase()) {
                        continue;
                    }
                    let track_id = uuid::Uuid::new_v4().to_string();
                    conn.execute(
                        "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms, video_id, _hlc) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        rusqlite::params![track_id, target_id, name, next_position, duration_ms, video_id, hlc],
                    )?;
                    existing_names.push(name.to_lowercase());
                    next_position += 1;
                }

                // Union tags from source onto target
                let mut stmt = conn
                    .prepare("SELECT tag_id FROM discovery_release_tags WHERE release_id = ?1")?;
                let tag_ids: Vec<String> = stmt
                    .query_map([source_id.as_str()], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                for tag_id in &tag_ids {
                    conn.execute(
                        "INSERT OR IGNORE INTO discovery_release_tags (release_id, tag_id, _hlc) VALUES (?1, ?2, ?3)",
                        rusqlite::params![target_id, tag_id, hlc],
                    )?;
                }

                // Move playlist memberships from source to target
                conn.execute(
                    "UPDATE OR IGNORE playlist_discovery_releases SET release_id = ?1, _hlc = ?2 WHERE release_id = ?3",
                    rusqlite::params![target_id, hlc, source_id],
                )?;
                // Delete any remaining (duplicates that couldn't be moved due to PK conflict)
                conn.execute(
                    "DELETE FROM playlist_discovery_releases WHERE release_id = ?1",
                    [source_id.as_str()],
                )?;

                // Collect notes
                let source_notes: Option<String> = conn.query_row(
                    "SELECT notes FROM discovery_releases WHERE id = ?1",
                    [source_id.as_str()],
                    |row| row.get(0),
                )?;
                if let Some(notes) = source_notes {
                    if !notes.is_empty() && !all_notes.contains(&notes) {
                        all_notes.push(notes);
                    }
                }

                // Delete source release (cascading deletes handle tracks/tags)
                dirty::record_tombstone(&conn, buckets::DISCOVERY_RELEASES, source_id, &hlc)?;
                conn.execute(
                    "DELETE FROM discovery_releases WHERE id = ?1",
                    [source_id.as_str()],
                )?;
            }

            // Update target: date_modified and concatenated notes
            let merged_notes = if all_notes.len() > 1 {
                Some(all_notes.join("\n\n"))
            } else {
                target_notes
            };

            conn.execute(
                "UPDATE discovery_releases SET date_modified = ?1, notes = ?2, _hlc = ?3 WHERE id = ?4",
                rusqlite::params![now, merged_notes, hlc, target_id],
            )?;
            dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
            dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACKS)?;
            dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;
            dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
        }

        drop(conn);
        self.get_release(target_id)
    }

    /// Update missing track durations by matching fetched tracks by name (case-insensitive).
    /// Only fills in `duration_ms` for tracks that currently have `NULL` duration.
    pub fn update_track_durations(
        &self,
        release_id: &str,
        fetched_tracks: &[FetchedTrack],
    ) -> Result<()> {
        if fetched_tracks.is_empty() {
            return Ok(());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            "SELECT id, name FROM discovery_tracks WHERE release_id = ?1 AND duration_ms IS NULL",
        )?;
        let null_tracks: Vec<(String, String)> = stmt
            .query_map([release_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (track_id, track_name) in &null_tracks {
            if let Some(fetched) = fetched_tracks
                .iter()
                .find(|ft| ft.name.eq_ignore_ascii_case(track_name))
            {
                if let Some(duration_ms) = fetched.duration_ms {
                    conn.execute(
                        "UPDATE discovery_tracks SET duration_ms = ?1 WHERE id = ?2",
                        rusqlite::params![duration_ms, track_id],
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Get the stored `video_id` for a specific track by release ID and position.
    pub fn get_video_id_for_track(
        &self,
        release_id: &str,
        track_position: i32,
    ) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let result = conn.query_row(
            "SELECT video_id FROM discovery_tracks WHERE release_id = ?1 AND position = ?2",
            rusqlite::params![release_id, track_position],
            |row| row.get::<_, Option<String>>(0),
        );

        match result {
            Ok(video_id) => Ok(video_id),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Get all stored `(position, video_id)` pairs for a release, ordered by position.
    pub fn get_all_video_ids_for_release(&self, release_id: &str) -> Result<Vec<(i32, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;
        let mut stmt = conn.prepare(
            "SELECT position, video_id FROM discovery_tracks
             WHERE release_id = ?1 AND video_id IS NOT NULL ORDER BY position",
        )?;
        let rows = stmt
            .query_map([release_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Backfill NULL `video_id` values from fetched metadata, matching by position.
    pub fn update_track_video_ids(
        &self,
        release_id: &str,
        fetched_tracks: &[FetchedTrack],
    ) -> Result<()> {
        if fetched_tracks.is_empty() {
            return Ok(());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            "SELECT id, position FROM discovery_tracks WHERE release_id = ?1 AND video_id IS NULL",
        )?;
        let null_tracks: Vec<(String, i32)> = stmt
            .query_map([release_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (track_id, position) in &null_tracks {
            if let Some(fetched) = fetched_tracks.iter().find(|ft| ft.position == *position) {
                if let Some(ref vid) = fetched.video_id {
                    conn.execute(
                        "UPDATE discovery_tracks SET video_id = ?1 WHERE id = ?2",
                        rusqlite::params![vid, track_id],
                    )?;
                }
            }
        }

        Ok(())
    }
}
