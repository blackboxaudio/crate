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

use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;
use crate::services::cloud_sync::hlc::{self, Hlc, NodeId};

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

/// One-time backfill: give every still-unstamped (`_hlc = ''`) row a real HLC so a
/// pre-sync library can be pushed deterministically. Each row's wall-clock is
/// derived from its own `date_modified`/`date_added` (timestamp-less tables use 0),
/// `counter = 0`, and this device's `node_id`. Gated by
/// `sync_state['initial_stamp_done']` so it runs exactly once; invoked on the first
/// push (and directly by tests).
pub fn stamp_unstamped_rows(conn: &Connection, node_id: NodeId) -> Result<()> {
    let done: Option<String> = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'initial_stamp_done'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if done.as_deref() == Some("1") {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;

    // Tables with no timestamp column: all unstamped rows share the epoch HLC.
    let epoch = Hlc::new(0, 0, node_id).format();
    for table in [
        "cues",
        "tags",
        "tag_categories",
        "track_tags",
        "discovery_release_tags",
        "discovery_release_sources",
        "discovery_tracks",
        "library_roots",
    ] {
        tx.execute(
            &format!("UPDATE {table} SET _hlc = ?1 WHERE _hlc = ''"),
            [&epoch],
        )?;
    }

    // Timestamped tables: derive each row's wall-clock from its date column(s).
    stamp_timestamped(
        &tx,
        "tracks",
        &["date_modified", "date_added"],
        &["id"],
        node_id,
    )?;
    stamp_timestamped(
        &tx,
        "playlists",
        &["date_modified", "date_created"],
        &["id"],
        node_id,
    )?;
    stamp_timestamped(
        &tx,
        "discovery_releases",
        &["date_modified", "date_added"],
        &["id"],
        node_id,
    )?;
    stamp_timestamped(
        &tx,
        "playlist_tracks",
        &["date_added"],
        &["playlist_id", "track_id"],
        node_id,
    )?;
    stamp_timestamped(
        &tx,
        "playlist_discovery_releases",
        &["date_added"],
        &["playlist_id", "release_id"],
        node_id,
    )?;
    stamp_timestamped(
        &tx,
        "followed_sources",
        &["date_modified", "date_added"],
        &["id"],
        node_id,
    )?;

    tx.execute(
        "INSERT INTO sync_state (key, value) VALUES ('initial_stamp_done', '1')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

/// Stamp the unstamped rows of one timestamped table. `date_cols` are tried in
/// order (first parseable wins); `pk_cols` are the 1 or 2 PK columns used to target
/// the UPDATE.
fn stamp_timestamped(
    conn: &Connection,
    table: &str,
    date_cols: &[&str],
    pk_cols: &[&str],
    node: NodeId,
) -> Result<()> {
    let n_pk = pk_cols.len();
    let n_date = date_cols.len();
    let select_cols: Vec<&str> = pk_cols
        .iter()
        .copied()
        .chain(date_cols.iter().copied())
        .collect();
    let select_sql = format!(
        "SELECT {} FROM {} WHERE _hlc = ''",
        select_cols.join(", "),
        table
    );

    let rows: Vec<(Vec<String>, u64)> = {
        let mut stmt = conn.prepare(&select_sql)?;
        let mapped = stmt.query_map([], |r| {
            let mut pks = Vec::with_capacity(n_pk);
            for i in 0..n_pk {
                pks.push(r.get::<_, String>(i)?);
            }
            let mut wall = 0u64;
            for j in 0..n_date {
                let date: Option<String> = r.get(n_pk + j)?;
                if let Some(s) = date {
                    let ms = parse_rfc3339_ms(&s);
                    if ms > 0 {
                        wall = ms;
                        break;
                    }
                }
            }
            Ok((pks, wall))
        })?;
        mapped.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let where_clause = pk_cols
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{c} = ?{}", i + 2))
        .collect::<Vec<_>>()
        .join(" AND ");
    let update_sql = format!("UPDATE {table} SET _hlc = ?1 WHERE {where_clause}");

    for (pks, wall) in rows {
        let hlc = Hlc::new(wall, 0, node).format();
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(n_pk + 1);
        params.push(&hlc);
        for p in &pks {
            params.push(p);
        }
        conn.execute(&update_sql, params.as_slice())?;
    }
    Ok(())
}

/// RFC3339 → milliseconds since the Unix epoch, or 0 if unparseable.
fn parse_rfc3339_ms(s: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp_millis().max(0) as u64)
        .unwrap_or(0)
}
