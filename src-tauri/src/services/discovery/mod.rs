use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
    DiscoveryTrack, Tag,
};

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
        let source_type = create
            .source_type
            .unwrap_or_else(|| detect_source_type(&create.url));

        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, artist, title, label, release_date, artwork_url, notes, date_added, date_modified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                id,
                create.url,
                source_type,
                create.artist,
                create.title,
                create.label,
                create.release_date,
                create.artwork_url,
                create.notes,
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
            url: create.url,
            source_type,
            artist: create.artist,
            title: create.title,
            label: create.label,
            release_date: create.release_date,
            artwork_url: create.artwork_url,
            artwork_path: None,
            notes: create.notes,
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
            "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, artwork_path, notes, date_added, date_modified
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
                    date_added: row.get(10)?,
                    date_modified: row.get(11)?,
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
            "SELECT DISTINCT dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label, dr.release_date, dr.artwork_url, dr.artwork_path, dr.notes, dr.date_added, dr.date_modified
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
                    date_added: row.get(10)?,
                    date_modified: row.get(11)?,
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

}

fn detect_source_type(url: &str) -> String {
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
