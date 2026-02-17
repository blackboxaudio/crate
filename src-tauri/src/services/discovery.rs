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

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let source_type = create.source_type.unwrap_or_else(|| "other".to_string());

        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, artist, title, label, release_date, artwork_url, status, notes, date_added, date_modified) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                id,
                create.url,
                source_type,
                create.artist,
                create.title,
                create.label,
                create.release_date,
                create.artwork_url,
                "unlistened",
                create.notes,
                now,
                now,
            ],
        )?;

        if let Some(tracks) = create.tracks {
            for track in tracks {
                let track_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![track_id, id, track.name, track.position, track.duration_ms],
                )?;
            }
        }

        drop(conn);
        self.get_release(&id)
    }

    pub fn get_release(&self, id: &str) -> Result<DiscoveryRelease> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut release = conn.query_row(
            "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, artwork_path, status, notes, date_added, date_modified FROM discovery_releases WHERE id = ?1",
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
                    status: row.get(9)?,
                    notes: row.get(10)?,
                    date_added: row.get(11)?,
                    date_modified: row.get(12)?,
                    tracks: Vec::new(),
                    tags: Vec::new(),
                })
            },
        )?;

        self.load_release_details(&conn, &mut release)?;

        Ok(release)
    }

    pub fn get_releases(&self, filter: Option<DiscoveryFilter>) -> Result<Vec<DiscoveryRelease>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut sql =
            String::from("SELECT DISTINCT r.id, r.url, r.source_type, r.artist, r.title, r.label, r.release_date, r.artwork_url, r.artwork_path, r.status, r.notes, r.date_added, r.date_modified FROM discovery_releases r");
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref f) = filter {
            if let Some(ref tag_ids) = f.tag_ids {
                if !tag_ids.is_empty() {
                    sql.push_str(
                        " INNER JOIN discovery_release_tags drt ON r.id = drt.release_id",
                    );

                    let placeholders: Vec<String> = tag_ids
                        .iter()
                        .map(|_| {
                            let p = format!("?{}", param_idx);
                            param_idx += 1;
                            p
                        })
                        .collect();

                    conditions.push(format!("drt.tag_id IN ({})", placeholders.join(", ")));

                    for tag_id in tag_ids {
                        params.push(Box::new(tag_id.clone()));
                    }

                    let is_and_mode = f
                        .tag_filter_mode
                        .as_deref()
                        .map(|m| m == "and")
                        .unwrap_or(false);

                    if is_and_mode {
                        sql = format!(
                            "SELECT DISTINCT r.id, r.url, r.source_type, r.artist, r.title, r.label, r.release_date, r.artwork_url, r.artwork_path, r.status, r.notes, r.date_added, r.date_modified FROM discovery_releases r INNER JOIN discovery_release_tags drt ON r.id = drt.release_id WHERE drt.tag_id IN ({}) GROUP BY r.id HAVING COUNT(DISTINCT drt.tag_id) = ?{}",
                            placeholders.join(", "),
                            param_idx
                        );
                        params.push(Box::new(tag_ids.len() as i32));
                        param_idx += 1;
                        conditions.clear();
                    }
                }
            }

            if let Some(ref status) = f.status {
                conditions.push(format!("r.status = ?{}", param_idx));
                params.push(Box::new(status.clone()));
                param_idx += 1;
            }

            if let Some(ref search) = f.search {
                let pattern = format!("%{}%", search);
                conditions.push(format!(
                    "(r.artist LIKE ?{p1} OR r.title LIKE ?{p2} OR r.label LIKE ?{p3} OR r.notes LIKE ?{p4} OR r.url LIKE ?{p5})",
                    p1 = param_idx,
                    p2 = param_idx + 1,
                    p3 = param_idx + 2,
                    p4 = param_idx + 3,
                    p5 = param_idx + 4,
                ));
                for _ in 0..5 {
                    params.push(Box::new(pattern.clone()));
                    param_idx += 1;
                }
            }
        }

        if !conditions.is_empty() {
            if sql.contains("WHERE") {
                sql.push_str(&format!(" AND {}", conditions.join(" AND ")));
            } else {
                sql.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
            }
        }

        if !sql.contains("GROUP BY") {
            sql.push_str(" ORDER BY r.date_added DESC");
        } else {
            sql.push_str(" ORDER BY r.date_added DESC");
        }

        let mut stmt = conn.prepare(&sql)?;

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

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
                    status: row.get(9)?,
                    notes: row.get(10)?,
                    date_added: row.get(11)?,
                    date_modified: row.get(12)?,
                    tracks: Vec::new(),
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for release in &mut releases {
            self.load_release_details(&conn, release)?;
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

        if let Some(ref artist) = update.artist {
            conn.execute(
                "UPDATE discovery_releases SET artist = ?1 WHERE id = ?2",
                rusqlite::params![artist, id],
            )?;
        }

        if let Some(ref title) = update.title {
            conn.execute(
                "UPDATE discovery_releases SET title = ?1 WHERE id = ?2",
                rusqlite::params![title, id],
            )?;
        }

        if let Some(ref label) = update.label {
            conn.execute(
                "UPDATE discovery_releases SET label = ?1 WHERE id = ?2",
                rusqlite::params![label, id],
            )?;
        }

        if let Some(ref release_date) = update.release_date {
            conn.execute(
                "UPDATE discovery_releases SET release_date = ?1 WHERE id = ?2",
                rusqlite::params![release_date, id],
            )?;
        }

        if let Some(ref artwork_url) = update.artwork_url {
            conn.execute(
                "UPDATE discovery_releases SET artwork_url = ?1 WHERE id = ?2",
                rusqlite::params![artwork_url, id],
            )?;
        }

        if let Some(ref artwork_path) = update.artwork_path {
            conn.execute(
                "UPDATE discovery_releases SET artwork_path = ?1 WHERE id = ?2",
                rusqlite::params![artwork_path, id],
            )?;
        }

        if let Some(ref notes) = update.notes {
            conn.execute(
                "UPDATE discovery_releases SET notes = ?1 WHERE id = ?2",
                rusqlite::params![notes, id],
            )?;
        }

        if let Some(ref status) = update.status {
            conn.execute(
                "UPDATE discovery_releases SET status = ?1 WHERE id = ?2",
                rusqlite::params![status, id],
            )?;
        }

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE discovery_releases SET date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;

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

        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "DELETE FROM discovery_releases WHERE id IN ({})",
            placeholders.join(", ")
        );

        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();

        conn.execute(&sql, params.as_slice())?;
        Ok(())
    }

    pub fn set_status(&self, id: &str, status: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE discovery_releases SET status = ?1, date_modified = ?2 WHERE id = ?3",
            rusqlite::params![status, now, id],
        )?;

        Ok(())
    }

    fn load_release_details(
        &self,
        conn: &Connection,
        release: &mut DiscoveryRelease,
    ) -> Result<()> {
        let mut stmt = conn.prepare(
            "SELECT id, release_id, name, position, duration_ms FROM discovery_tracks WHERE release_id = ?1 ORDER BY position",
        )?;

        release.tracks = stmt
            .query_map([&release.id], |row| {
                Ok(DiscoveryTrack {
                    id: row.get(0)?,
                    release_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    duration_ms: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut stmt = conn.prepare(
            "SELECT t.id, t.category_id, t.name, t.color, t.sort_order FROM tags t INNER JOIN discovery_release_tags drt ON t.id = drt.tag_id WHERE drt.release_id = ?1 ORDER BY t.sort_order, t.name",
        )?;

        release.tags = stmt
            .query_map([&release.id], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(())
    }
}
