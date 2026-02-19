pub mod metadata;
pub mod streams;

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
    DiscoveryTrack, DiscoveryTrackCreate, Tag,
};

use metadata::FetchedTrack;
use streams::StreamInfo;

pub struct DiscoveryService {
    conn: Arc<Mutex<Connection>>,
}

impl DiscoveryService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn create_release(&self, create: DiscoveryReleaseCreate) -> Result<DiscoveryRelease> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let normalized_url = normalize_url(&create.url);
        let source_type = create
            .source_type
            .unwrap_or_else(|| detect_source_type(&normalized_url));

        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, artist, title, label, release_date, artwork_url, notes, parent_url, date_added, date_modified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                now,
                now,
            ],
        )?;

        // Insert tracks if provided
        let mut tracks = Vec::new();
        if let Some(track_creates) = create.tracks {
            for tc in track_creates {
                let track_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![track_id, id, tc.name, tc.position, tc.duration_ms],
                )?;
                tracks.push(DiscoveryTrack {
                    id: track_id,
                    release_id: id.clone(),
                    name: tc.name,
                    position: tc.position,
                    duration_ms: tc.duration_ms,
                });
            }
        }

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
            date_added: now.clone(),
            date_modified: now,
            tracks,
            tags: Vec::new(),
        })
    }

    pub fn get_release(&self, id: &str) -> Result<DiscoveryRelease> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut release = conn.query_row(
            "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, artwork_path, notes, parent_url, date_added, date_modified
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
                    date_added: row.get(11)?,
                    date_modified: row.get(12)?,
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
            "SELECT id, release_id, name, position, duration_ms FROM discovery_tracks WHERE release_id = ?1 ORDER BY position",
        )?;
        release.tracks = stmt
            .query_map([id], |row| {
                Ok(DiscoveryTrack {
                    id: row.get(0)?,
                    release_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    duration_ms: row.get(4)?,
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

        Ok(release)
    }

    pub fn get_releases(&self, filter: Option<DiscoveryFilter>) -> Result<Vec<DiscoveryRelease>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let filter = filter.unwrap_or_default();

        // Build query with optional filters
        let mut sql = String::from(
            "SELECT DISTINCT dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label, dr.release_date, dr.artwork_url, dr.artwork_path, dr.notes, dr.parent_url, dr.date_added, dr.date_modified
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
                "(dr.artist LIKE ? OR dr.title LIKE ? OR dr.label LIKE ? OR dr.notes LIKE ?)"
                    .to_string(),
            );
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

        // Batch load tracks for all releases
        let release_ids: Vec<String> = releases.iter().map(|r| r.id.clone()).collect();
        let placeholders = release_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let mut stmt = conn.prepare(&format!(
            "SELECT id, release_id, name, position, duration_ms FROM discovery_tracks WHERE release_id IN ({placeholders}) ORDER BY position"
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

    pub fn update_release(
        &self,
        id: &str,
        update: DiscoveryReleaseUpdate,
    ) -> Result<DiscoveryRelease> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut set_clauses = vec!["date_modified = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        if let Some(ref artist) = update.artist {
            set_clauses.push("artist = ?".to_string());
            params.push(Box::new(artist.clone()));
        }
        if let Some(ref title) = update.title {
            set_clauses.push("title = ?".to_string());
            params.push(Box::new(title.clone()));
        }
        if let Some(ref label) = update.label {
            set_clauses.push("label = ?".to_string());
            params.push(Box::new(label.clone()));
        }
        if let Some(ref release_date) = update.release_date {
            set_clauses.push("release_date = ?".to_string());
            params.push(Box::new(release_date.clone()));
        }
        if let Some(ref artwork_url) = update.artwork_url {
            set_clauses.push("artwork_url = ?".to_string());
            params.push(Box::new(artwork_url.clone()));
        }
        if let Some(ref artwork_path) = update.artwork_path {
            set_clauses.push("artwork_path = ?".to_string());
            params.push(Box::new(artwork_path.clone()));
        }
        if let Some(ref notes) = update.notes {
            set_clauses.push("notes = ?".to_string());
            params.push(Box::new(notes.clone()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!(
            "UPDATE discovery_releases SET {} WHERE id = ?",
            set_clauses.join(", ")
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())?;

        drop(conn);
        self.get_release(id)
    }

    pub fn delete_release(&self, id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute("DELETE FROM discovery_releases WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_releases(&self, ids: Vec<String>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!("DELETE FROM discovery_releases WHERE id IN ({placeholders})");
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        conn.execute(&sql, params.as_slice())?;

        Ok(())
    }

    pub fn assign_tags(&self, release_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for release_id in &release_ids {
            for tag_id in &tag_ids {
                conn.execute(
                    "INSERT OR IGNORE INTO discovery_release_tags (release_id, tag_id) VALUES (?1, ?2)",
                    rusqlite::params![release_id, tag_id],
                )?;
            }
        }

        Ok(())
    }

    pub fn remove_tags(&self, release_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for release_id in &release_ids {
            for tag_id in &tag_ids {
                conn.execute(
                    "DELETE FROM discovery_release_tags WHERE release_id = ?1 AND tag_id = ?2",
                    rusqlite::params![release_id, tag_id],
                )?;
            }
        }

        Ok(())
    }

    /// Find existing releases that may overlap with the given metadata.
    /// Checks by parent_url match and artist+title match.
    pub fn find_matching_releases(
        &self,
        artist: Option<&str>,
        title: Option<&str>,
        parent_url: Option<&str>,
    ) -> Result<Vec<DiscoveryRelease>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut matched_ids: Vec<String> = Vec::new();

        // Check 1: If parent_url provided, find releases whose url matches the parent_url
        if let Some(p_url) = parent_url {
            let normalized = normalize_url(p_url);
            let mut stmt = conn.prepare(
                "SELECT id FROM discovery_releases WHERE url = ?1",
            )?;
            let ids: Vec<String> = stmt
                .query_map([&normalized], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            matched_ids.extend(ids);
        }

        // Check 2: If parent_url provided, find releases with the same parent_url
        if let Some(p_url) = parent_url {
            let normalized = normalize_url(p_url);
            let mut stmt = conn.prepare(
                "SELECT id FROM discovery_releases WHERE parent_url = ?1",
            )?;
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
        matched_ids
            .iter()
            .map(|id| self.get_release(id))
            .collect()
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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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
            let mut next_position = max_position + 1;

            for track in &tracks {
                if existing_names.contains(&track.name.to_lowercase()) {
                    continue;
                }
                let track_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![track_id, release_id, track.name, next_position, track.duration_ms],
                )?;
                next_position += 1;
            }

            // Update date_modified
            conn.execute(
                "UPDATE discovery_releases SET date_modified = ?1 WHERE id = ?2",
                rusqlite::params![now, release_id],
            )?;
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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        {
            let now = chrono::Utc::now().to_rfc3339();

            // Get existing target track names for dedup
            let mut stmt = conn.prepare(
                "SELECT LOWER(name), MAX(position) FROM discovery_tracks WHERE release_id = ?1 GROUP BY LOWER(name)",
            )?;
            let existing: Vec<(String, i32)> = stmt
                .query_map([target_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let mut existing_names: Vec<String> =
                existing.iter().map(|(n, _)| n.clone()).collect();
            let mut next_position =
                existing.iter().map(|(_, p)| *p).max().unwrap_or(0) + 1;

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
                    "SELECT name, position, duration_ms FROM discovery_tracks WHERE release_id = ?1 ORDER BY position",
                )?;
                let source_tracks: Vec<(String, i32, Option<i64>)> = stmt
                    .query_map([source_id.as_str()], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                for (name, _, duration_ms) in &source_tracks {
                    if existing_names.contains(&name.to_lowercase()) {
                        continue;
                    }
                    let track_id = uuid::Uuid::new_v4().to_string();
                    conn.execute(
                        "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![track_id, target_id, name, next_position, duration_ms],
                    )?;
                    existing_names.push(name.to_lowercase());
                    next_position += 1;
                }

                // Union tags from source onto target
                let mut stmt = conn.prepare(
                    "SELECT tag_id FROM discovery_release_tags WHERE release_id = ?1",
                )?;
                let tag_ids: Vec<String> = stmt
                    .query_map([source_id.as_str()], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                for tag_id in &tag_ids {
                    conn.execute(
                        "INSERT OR IGNORE INTO discovery_release_tags (release_id, tag_id) VALUES (?1, ?2)",
                        rusqlite::params![target_id, tag_id],
                    )?;
                }

                // Move playlist memberships from source to target
                conn.execute(
                    "UPDATE OR IGNORE playlist_discovery_releases SET release_id = ?1 WHERE release_id = ?2",
                    rusqlite::params![target_id, source_id],
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
                "UPDATE discovery_releases SET date_modified = ?1, notes = ?2 WHERE id = ?3",
                rusqlite::params![now, merged_notes, target_id],
            )?;
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
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

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

    /// Get a clone of the database connection Arc for use in background tasks.
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    /// Get a cached stream URL for a specific track position, if it exists and hasn't expired.
    pub fn get_cached_stream(
        &self,
        release_id: &str,
        track_position: i32,
    ) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        let result = conn.query_row(
            "SELECT stream_url FROM discovery_stream_cache
             WHERE release_id = ?1 AND track_position = ?2 AND expires_at > ?3",
            rusqlite::params![release_id, track_position, now],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(url) => Ok(Some(url)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Cache stream URLs for a release, replacing any existing entries.
    pub fn cache_streams(&self, release_id: &str, streams: &[StreamInfo]) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for stream in streams {
            conn.execute(
                "INSERT OR REPLACE INTO discovery_stream_cache (release_id, track_position, stream_url, expires_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    release_id,
                    stream.track_position,
                    stream.stream_url,
                    stream.expires_at,
                ],
            )?;
        }

        Ok(())
    }

    /// Get the cached SoundCloud client_id, if one exists and was fetched within the last 24 hours.
    pub fn get_cached_sc_client_id(&self) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let result = conn.query_row(
            "SELECT client_id FROM discovery_sc_client_id_cache WHERE id = 1 AND fetched_at > ?1",
            rusqlite::params![cutoff],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(cid) => Ok(Some(cid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Invalidate cached stream URLs for a release, forcing re-fetch on next play.
    pub fn invalidate_stream_cache(&self, release_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute(
            "DELETE FROM discovery_stream_cache WHERE release_id = ?1",
            [release_id],
        )?;

        Ok(())
    }

    /// Cache a SoundCloud client_id.
    pub fn cache_sc_client_id(&self, client_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovery_sc_client_id_cache (id, client_id, fetched_at) VALUES (1, ?1, ?2)",
            rusqlite::params![client_id, now],
        )?;

        Ok(())
    }
}

/// Normalize a URL for consistent storage and deduplication.
/// - Lowercases the domain (not path)
/// - Strips trailing slashes
/// - Removes common tracking query parameters
pub(crate) fn normalize_url(url: &str) -> String {
    let url = url.trim();

    // Parse into parts: scheme, domain, path+query
    let (scheme, rest) = match url.find("://") {
        Some(i) => (&url[..i], &url[i + 3..]),
        None => return url.to_string(),
    };

    let (authority, path_and_query) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    // Lowercase the authority (domain + optional port)
    let authority_lower = authority.to_lowercase();

    // Split path from query
    let (path, query) = match path_and_query.find('?') {
        Some(i) => (&path_and_query[..i], Some(&path_and_query[i + 1..])),
        None => (path_and_query, None),
    };

    // Strip trailing slashes from path (but keep at least "/")
    let path = path.trim_end_matches('/');
    let path = if path.is_empty() { "" } else { path };

    // Filter out tracking query params
    let tracking_params: &[&str] = &[
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_content",
        "utm_term",
        "ref",
        "fbclid",
        "si",
        "feature",
    ];

    let filtered_query = query
        .map(|q| {
            q.split('&')
                .filter(|param| {
                    let key = param.split('=').next().unwrap_or("");
                    !tracking_params.contains(&key)
                })
                .collect::<Vec<_>>()
                .join("&")
        })
        .filter(|q| !q.is_empty());

    match filtered_query {
        Some(q) => format!("{scheme}://{authority_lower}{path}?{q}"),
        None => format!("{scheme}://{authority_lower}{path}"),
    }
}

pub(crate) fn detect_source_type(url: &str) -> String {
    let url_lower = url.to_lowercase();
    if url_lower.contains("bandcamp.com") {
        "bandcamp".to_string()
    } else if url_lower.contains("soundcloud.com") {
        "soundcloud".to_string()
    } else if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
        "youtube".to_string()
    } else if url_lower.contains("discogs.com") {
        "discogs".to_string()
    } else {
        "other".to_string()
    }
}
