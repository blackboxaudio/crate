//! Per-mutation change-tracking hooks.
//!
//! Every place that mutates a synced table calls into here to (a) obtain a fresh
//! HLC to stamp the row's `_hlc`, (b) mark the affected bucket dirty, and (c) on
//! a hard delete, record a tombstone. Call [`next_hlc`] **once per logical
//! mutation**: a method that touches N rows stamps them all with the same HLC
//! (they changed in one event), which is both correct and cheap.
//!
//! Deletes record a tombstone only for the directly-deleted entity. Cascade
//! children (via `ON DELETE CASCADE`) converge on peers through the same
//! symmetric cascade when the parent's delete is merged, so they need no
//! tombstone of their own.
//!
//! All hooks here are **best-effort**: if the sync bookkeeping is unavailable
//! (e.g. the sync tables aren't present yet), they log a warning and degrade
//! gracefully — a row may be left "unstamped" — rather than failing the user's
//! underlying mutation. Local-first edits must never break because sync hiccuped.

use rusqlite::Connection;

use crate::error::Result;
use crate::services::cloud_sync::hlc::{self, Hlc};

use super::buckets;

/// Advance the persistent HLC by one local event and return its string form.
/// Must be called while holding the connection mutex (it reads + writes
/// `sync_state`). Never call after the mutex guard is dropped.
///
/// Best-effort: on failure it returns the empty "never stamped" sentinel (so the
/// row is left for first-sync stamping) instead of failing the caller's mutation.
pub fn next_hlc(conn: &Connection) -> Result<String> {
    match try_next_hlc(conn) {
        Ok(h) => Ok(h),
        Err(e) => {
            log::warn!("cloud_sync: HLC stamp skipped, row left unstamped: {e}");
            Ok(String::new())
        }
    }
}

fn try_next_hlc(conn: &Connection) -> Result<String> {
    let mut clock = Hlc::load(conn)?;
    clock.tick(hlc::now_ms());
    clock.persist(conn)?;
    Ok(clock.format())
}

/// Mark a bucket dirty so the next push re-serializes + re-uploads it.
/// Best-effort — a failure here never propagates to the caller's mutation.
pub fn mark_dirty(conn: &Connection, bucket: &str) -> Result<()> {
    if let Err(e) = conn.execute(
        "INSERT INTO sync_dirty_buckets (bucket, marked_at) VALUES (?1, ?2)
         ON CONFLICT(bucket) DO UPDATE SET marked_at = excluded.marked_at",
        rusqlite::params![bucket, chrono::Utc::now().to_rfc3339()],
    ) {
        log::warn!("cloud_sync: mark_dirty({bucket}) skipped: {e}");
    }
    Ok(())
}

/// Mark the track shard for `track_id` dirty.
pub fn mark_dirty_track(conn: &Connection, track_id: &str) -> Result<()> {
    mark_dirty(conn, &buckets::bucket_for_track_id(track_id))
}

/// Mark every distinct track shard touched by `track_ids` dirty.
pub fn mark_dirty_track_shards(conn: &Connection, track_ids: &[String]) -> Result<()> {
    let mut shards: std::collections::HashSet<String> = std::collections::HashSet::new();
    for id in track_ids {
        shards.insert(buckets::bucket_for_track_id(id));
    }
    for shard in shards {
        mark_dirty(conn, &shard)?;
    }
    Ok(())
}

/// Record a tombstone for a hard-deleted row so the delete propagates.
/// `entity_type` is the bucket/entity name; `entity_id` is the row PK, or the
/// composite junction id from [`junction_entity_id`].
pub fn record_tombstone(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    hlc: &str,
) -> Result<()> {
    if let Err(e) = conn.execute(
        "INSERT INTO sync_tombstones (entity_type, entity_id, _hlc) VALUES (?1, ?2, ?3)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET _hlc = excluded._hlc",
        rusqlite::params![entity_type, entity_id, hlc],
    ) {
        log::warn!("cloud_sync: tombstone({entity_type}/{entity_id}) skipped: {e}");
    }
    Ok(())
}

/// Stamp a whitelisted setting key (stored in `sync_state` as `setting_hlc:<key>`)
/// and mark the settings bucket dirty. Best-effort — never fails `set_setting`.
pub fn stamp_setting(conn: &Connection, key: &str) {
    if let Err(e) = try_stamp_setting(conn, key) {
        log::warn!("cloud_sync: setting stamp for {key} skipped: {e}");
    }
}

fn try_stamp_setting(conn: &Connection, key: &str) -> Result<()> {
    let hlc = try_next_hlc(conn)?;
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![format!("setting_hlc:{key}"), hlc],
    )?;
    mark_dirty(conn, buckets::SETTINGS)?;
    Ok(())
}

/// Canonical composite-key identity for a junction row: the two ids joined by
/// `|` in fixed PK-declaration column order. UUIDs never contain `|`, so the
/// split is unambiguous. Every producer and consumer MUST use this helper.
pub fn junction_entity_id(a: &str, b: &str) -> String {
    format!("{a}|{b}")
}

/// Split a [`junction_entity_id`] back into its two parts.
pub fn split_junction_id(s: &str) -> Option<(&str, &str)> {
    s.split_once('|')
}
