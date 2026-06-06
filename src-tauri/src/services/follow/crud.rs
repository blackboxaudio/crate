//! Synchronous DB methods for `FollowService`. Synced tables (`followed_sources`,
//! `discovery_release_sources`, the `discovery_releases.is_new`/`surfaced_at` columns)
//! stamp `_hlc` + mark the bucket dirty; the per-device tables (`followed_source_state`,
//! `followed_source_releases`) do neither.

use rusqlite::OptionalExtension;

use super::{FollowService, SourceToCheck};
use crate::error::{CrateError, Result};
use crate::models::{FollowHealth, FollowedSource, FollowedSourceCreate};
use crate::services::cloud_sync::pipeline::{buckets, dirty};
use crate::services::discovery::{detect_source_type, normalize_url};

/// Full SELECT for a `FollowedSource`: the synced row joined to local watch state, with
/// the "new" count and most-recent surfaced release date computed inline.
const FOLLOW_SELECT: &str = "SELECT \
    fs.id, fs.url, fs.source_type, fs.follow_type, fs.name, fs.artwork_url, fs.artwork_path, \
    fs.enabled, fs.date_added, fs.date_modified, \
    st.last_checked_at, COALESCE(st.health, 'unknown'), st.last_error, \
    (SELECT COUNT(*) FROM followed_source_releases fsr \
       JOIN discovery_releases dr ON dr.id = fsr.release_id \
       WHERE fsr.source_id = fs.id AND fsr.status = 'surfaced' AND dr.is_new = 1), \
    (SELECT MAX(dr2.release_date) FROM followed_source_releases fsr2 \
       JOIN discovery_releases dr2 ON dr2.id = fsr2.release_id \
       WHERE fsr2.source_id = fs.id AND fsr2.status = 'surfaced') \
    FROM followed_sources fs \
    LEFT JOIN followed_source_state st ON st.source_id = fs.id";

const SOURCE_CHECK_SELECT: &str = "SELECT fs.id, fs.url, fs.source_type, fs.follow_type, fs.name, \
    COALESCE(st.baseline_established, 0) \
    FROM followed_sources fs \
    LEFT JOIN followed_source_state st ON st.source_id = fs.id";

fn map_follow_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FollowedSource> {
    Ok(FollowedSource {
        id: row.get(0)?,
        url: row.get(1)?,
        source_type: row.get(2)?,
        follow_type: row.get(3)?,
        name: row.get(4)?,
        artwork_url: row.get(5)?,
        artwork_path: row.get(6)?,
        enabled: row.get(7)?,
        date_added: row.get(8)?,
        date_modified: row.get(9)?,
        last_checked_at: row.get(10)?,
        health: row.get(11)?,
        last_error: row.get(12)?,
        new_count: row.get(13)?,
        last_release_at: row.get(14)?,
    })
}

fn map_source_to_check(row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceToCheck> {
    Ok(SourceToCheck {
        id: row.get(0)?,
        url: row.get(1)?,
        source_type: row.get(2)?,
        follow_type: row.get(3)?,
        name: row.get(4)?,
        baseline_established: row.get(5)?,
    })
}

impl FollowService {
    /// Insert a followed-source row + its local watch-state row. Idempotent on URL: an
    /// existing follow is re-enabled and returned. Does NOT scan — the caller
    /// establishes the baseline (see `watch::establish_baseline`).
    pub fn create_follow(&self, create: FollowedSourceCreate) -> Result<FollowedSource> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let normalized_url = normalize_url(&create.url);
        let source_type = create
            .source_type
            .unwrap_or_else(|| detect_source_type(&normalized_url));
        let follow_type = create.follow_type.unwrap_or_else(|| "artist".to_string());

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "INSERT INTO followed_sources \
                (id, url, source_type, follow_type, name, artwork_url, artwork_path, enabled, date_added, date_modified, _hlc) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 1, ?7, ?7, ?8) \
             ON CONFLICT(url) DO UPDATE SET enabled = 1, date_modified = ?7, _hlc = ?8",
            rusqlite::params![
                id,
                normalized_url,
                source_type,
                follow_type,
                create.name,
                create.artwork_url,
                now,
                hlc
            ],
        )?;
        dirty::mark_dirty(&conn, buckets::FOLLOWED_SOURCES)?;

        // A URL conflict keeps the existing id; resolve it before seeding state.
        let resolved_id: String = conn.query_row(
            "SELECT id FROM followed_sources WHERE url = ?1",
            [&normalized_url],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO followed_source_state (source_id) VALUES (?1)",
            [&resolved_id],
        )?;

        drop(conn);
        self.get_follow(&resolved_id)
    }

    pub fn get_follow(&self, id: &str) -> Result<FollowedSource> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let sql = format!("{FOLLOW_SELECT} WHERE fs.id = ?1");
        conn.query_row(&sql, [id], map_follow_row)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CrateError::Discovery(format!("Followed source not found: {id}"))
                }
                _ => CrateError::Database(e),
            })
    }

    pub fn list_follows(&self) -> Result<Vec<FollowedSource>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let sql = format!("{FOLLOW_SELECT} ORDER BY fs.date_added DESC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_follow_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Non-destructive unfollow: deletes the source (cascading away local state, seen
    /// rows, and provenance links) but leaves already-surfaced discovery releases.
    pub fn unfollow(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;
        dirty::record_tombstone(&conn, buckets::FOLLOWED_SOURCES, id, &hlc)?;
        conn.execute("DELETE FROM followed_sources WHERE id = ?1", [id])?;
        dirty::mark_dirty(&conn, buckets::FOLLOWED_SOURCES)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_SOURCES)?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<FollowedSource> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE followed_sources SET enabled = ?1, date_modified = ?2, _hlc = ?3 WHERE id = ?4",
            rusqlite::params![enabled, now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, buckets::FOLLOWED_SOURCES)?;
        drop(conn);
        self.get_follow(id)
    }

    /// Record the forward-looking baseline: every URL currently on the page becomes
    /// "known" (surfacing nothing), and the source is flipped to baseline-established.
    pub fn record_baseline(&self, source_id: &str, urls: &[String]) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        for url in urls {
            conn.execute(
                "INSERT OR IGNORE INTO followed_source_releases (source_id, seen_url, status, first_seen_at) \
                 VALUES (?1, ?2, 'baseline', ?3)",
                rusqlite::params![source_id, url, now],
            )?;
        }
        conn.execute(
            "UPDATE followed_source_state SET baseline_established = 1, last_checked_at = ?2, \
             last_success_at = ?2, health = 'ok', last_error = NULL, consecutive_failures = 0 \
             WHERE source_id = ?1",
            rusqlite::params![source_id, now],
        )?;
        Ok(())
    }

    pub fn mark_checked(
        &self,
        source_id: &str,
        health: FollowHealth,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let health_str = health.to_string();
        if matches!(health, FollowHealth::Ok) {
            conn.execute(
                "UPDATE followed_source_state SET last_checked_at = ?2, last_success_at = ?2, \
                 health = ?3, last_error = NULL, consecutive_failures = 0 WHERE source_id = ?1",
                rusqlite::params![source_id, now, health_str],
            )?;
        } else {
            conn.execute(
                "UPDATE followed_source_state SET last_checked_at = ?2, health = ?3, \
                 last_error = ?4, consecutive_failures = consecutive_failures + 1 WHERE source_id = ?1",
                rusqlite::params![source_id, now, health_str, error],
            )?;
        }
        Ok(())
    }

    /// URL → status map of everything seen under a source (baseline/surfaced/dismissed).
    pub fn get_seen_urls(
        &self,
        source_id: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let mut stmt =
            conn.prepare("SELECT seen_url, status FROM followed_source_releases WHERE source_id = ?1")?;
        let rows = stmt.query_map([source_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (u, s) = row?;
            map.insert(u, s);
        }
        Ok(map)
    }

    pub fn record_seen(
        &self,
        source_id: &str,
        url: &str,
        status: &str,
        release_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO followed_source_releases (source_id, seen_url, status, release_id, first_seen_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(source_id, seen_url) DO UPDATE SET status = ?3, release_id = COALESCE(?4, release_id)",
            rusqlite::params![source_id, url, status, release_id, now],
        )?;
        Ok(())
    }

    pub fn add_provenance(&self, release_id: &str, source_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO discovery_release_sources (release_id, source_id, _hlc) VALUES (?1, ?2, ?3)",
            rusqlite::params![release_id, source_id, hlc],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_SOURCES)?;
        Ok(())
    }

    /// Flag a freshly-surfaced release as new (is_new=1 + surfaced_at=now).
    pub fn mark_surfaced(&self, release_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE discovery_releases SET is_new = 1, surfaced_at = ?2, _hlc = ?3 WHERE id = ?1",
            rusqlite::params![release_id, now, hlc],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
        Ok(())
    }

    /// Manual override / auto-clear of the "new" flag.
    pub fn set_new_flag(&self, release_id: &str, is_new: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE discovery_releases SET is_new = ?2, _hlc = ?3 WHERE id = ?1",
            rusqlite::params![release_id, is_new, hlc],
        )?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASES)?;
        Ok(())
    }

    /// The discovery release id for a normalized URL, if one exists.
    pub fn release_id_for_url(&self, url: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let id = conn
            .query_row(
                "SELECT id FROM discovery_releases WHERE url = ?1",
                [url],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn enabled_sources(&self) -> Result<Vec<SourceToCheck>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let sql = format!("{SOURCE_CHECK_SELECT} WHERE fs.enabled = 1 ORDER BY fs.date_added");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_source_to_check)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn source_to_check(&self, id: &str) -> Result<SourceToCheck> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let sql = format!("{SOURCE_CHECK_SELECT} WHERE fs.id = ?1");
        conn.query_row(&sql, [id], map_source_to_check)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CrateError::Discovery(format!("Followed source not found: {id}"))
                }
                _ => CrateError::Database(e),
            })
    }
}
