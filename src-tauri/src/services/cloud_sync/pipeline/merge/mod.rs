//! The merge engine: fold a bucket's remote rows into local state, HLC as the
//! oracle. One transaction per bucket.
//!
//! ## Tie-breaks (must match [`super::rows`]'s serialize-time rule)
//! - **Entities — DELETE-WINS-TIE:** a delete at HLC `>=` the live row wins; a live
//!   upsert must be strictly `>` a tombstone to resurrect.
//! - **Junctions — ADD-WINS-TIE:** a delete wins only if strictly `>` the add; an
//!   add at HLC `>=` a tombstone resurrects. Ordering fields (`position`,
//!   `date_added`) are LWW; the link `_hlc` always advances to the max so the two
//!   devices' rows converge byte-for-byte.
//! - **Settings — LWW:** higher per-key HLC wins; the loser writes the winner's
//!   value AND its HLC verbatim (no fresh stamp, no dirty-mark → no push loop).
//!
//! HLCs are compared as **strings** (the `{wall:016x}-{counter:08x}-{node:08x}`
//! format is byte-lexicographic; `""` sorts below all). Never parse to compare.
//! Local state is re-read per remote row, so a bucket with duplicate or
//! out-of-order rows still merges idempotently.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{CrateError, Result};

use super::buckets::{Bucket, BucketKind};
use super::dirty;
use super::rows::{self, ParsedRow};

mod writers;

#[cfg(test)]
mod tests;

/// Merge a bucket's parsed remote rows into local state, in one transaction.
/// FK checks are deferred to commit so intra-bucket ordering (e.g. a child
/// playlist before its parent folder) is tolerated.
pub fn merge_bucket(conn: &Connection, bucket: &Bucket, rows: &[ParsedRow]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    match bucket.kind() {
        BucketKind::Settings => merge_settings(&tx, rows)?,
        BucketKind::Entity => {
            for row in rows {
                merge_entity_row(&tx, bucket, row)?;
            }
        }
        BucketKind::Junction => {
            for row in rows {
                merge_junction_row(&tx, bucket, row)?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

fn hlc_ge(a: &str, b: &str) -> bool {
    a >= b
}
fn hlc_gt(a: &str, b: &str) -> bool {
    a > b
}

fn select_live_hlc(tx: &Connection, bucket: &Bucket, cid: &str) -> Result<Option<String>> {
    let table = bucket.table();
    match bucket.pk_columns() {
        [_] => Ok(tx
            .query_row(
                &format!("SELECT _hlc FROM {table} WHERE id = ?1"),
                [cid],
                |r| r.get::<_, String>(0),
            )
            .optional()?),
        [c0, c1] => {
            let (a, b) = dirty::split_junction_id(cid)
                .ok_or_else(|| CrateError::CloudSync(format!("malformed junction id {cid:?}")))?;
            Ok(tx
                .query_row(
                    &format!("SELECT _hlc FROM {table} WHERE {c0} = ?1 AND {c1} = ?2"),
                    params![a, b],
                    |r| r.get::<_, String>(0),
                )
                .optional()?)
        }
        _ => Err(CrateError::CloudSync("unexpected PK arity".into())),
    }
}

fn select_tomb_hlc(tx: &Connection, bucket: &Bucket, cid: &str) -> Result<Option<String>> {
    Ok(tx
        .query_row(
            "SELECT _hlc FROM sync_tombstones WHERE entity_type = ?1 AND entity_id = ?2",
            params![bucket.entity_type(), cid],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

// ---------------------------------------------------------------------------
// Entities — DELETE-WINS-TIE
// ---------------------------------------------------------------------------

fn merge_entity_row(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<()> {
    let cid = rows::canonical_id(bucket, &row.value)?;
    let local_live = select_live_hlc(tx, bucket, &cid)?;
    let local_tomb = select_tomb_hlc(tx, bucket, &cid)?;

    if !row.deleted {
        // ---- remote LIVE (upsert) ----
        match (local_live, local_tomb) {
            // A tombstone at >= the remote's HLC keeps the entity deleted.
            (_, Some(t)) if hlc_ge(&t, &row.hlc) => {}
            // Tombstone strictly older than the remote → resurrect.
            (_, Some(_)) => {
                writers::delete_tombstone(tx, bucket, &cid)?;
                writers::upsert_entity(tx, bucket, row)?;
            }
            // Brand new.
            (None, None) => writers::upsert_entity(tx, bucket, row)?,
            // Present locally: LWW, remote wins only if strictly newer.
            (Some(l), None) => {
                if hlc_gt(&row.hlc, &l) {
                    writers::upsert_entity(tx, bucket, row)?;
                }
            }
        }
    } else {
        // ---- remote DELETE (tombstone) ---- DELETE-WINS-TIE (>=)
        match (local_live, local_tomb) {
            (Some(l), _) if hlc_ge(&row.hlc, &l) => {
                writers::hard_delete_entity(tx, bucket, &cid)?;
                writers::upsert_tombstone(tx, bucket, &cid, &row.hlc)?;
            }
            // Local live strictly newer than the delete → keep local, drop the delete.
            (Some(_), _) => {}
            // No local live: record/advance the tombstone (MAX inside upsert_tombstone),
            // so an older concurrent add stays suppressed.
            (None, _) => writers::upsert_tombstone(tx, bucket, &cid, &row.hlc)?,
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Junctions — ADD-WINS-TIE
// ---------------------------------------------------------------------------

fn merge_junction_row(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<()> {
    let cid = rows::canonical_id(bucket, &row.value)?;
    let local_live = select_live_hlc(tx, bucket, &cid)?;
    let local_tomb = select_tomb_hlc(tx, bucket, &cid)?;
    let ordered = matches!(
        bucket,
        Bucket::PlaylistTracks | Bucket::PlaylistDiscoveryReleases
    );

    if !row.deleted {
        // ---- remote LIVE (add) ---- ADD-WINS-TIE
        match (local_live, local_tomb) {
            // Already present: converge the ordering + clock.
            (Some(l), _) => {
                if ordered {
                    if hlc_gt(&row.hlc, &l) {
                        writers::upsert_junction_ordering(tx, bucket, row)?;
                    }
                } else {
                    let max_hlc = if hlc_gt(&row.hlc, &l) { &row.hlc } else { &l };
                    writers::advance_junction_hlc(tx, bucket, &cid, max_hlc)?;
                }
            }
            // Removed locally, but the add is at >= the removal → resurrect (add-wins-tie),
            // provided both endpoints still exist.
            (None, Some(t)) if hlc_ge(&row.hlc, &t) => {
                if writers::junction_endpoints_exist(tx, bucket, row)? {
                    writers::delete_tombstone(tx, bucket, &cid)?;
                    writers::insert_junction(tx, bucket, row)?;
                }
                // else: endpoints gone (cascade) — leave the removal in place.
            }
            // Add older than the removal → stays removed.
            (None, Some(_)) => {}
            // Brand new: insert iff endpoints exist, else skip the orphan.
            (None, None) => {
                if writers::junction_endpoints_exist(tx, bucket, row)? {
                    writers::insert_junction(tx, bucket, row)?;
                } else {
                    log::warn!(
                        "cloud_sync merge: skipping orphan {} row {cid}",
                        bucket.as_str()
                    );
                }
            }
        }
    } else {
        // ---- remote DELETE ---- delete only if STRICTLY newer than the add.
        match (local_live, local_tomb) {
            (Some(l), _) if hlc_gt(&row.hlc, &l) => {
                writers::delete_junction(tx, bucket, &cid)?;
                writers::upsert_tombstone(tx, bucket, &cid, &row.hlc)?;
            }
            // Local add at >= the delete → keep (add-wins-tie).
            (Some(_), _) => {}
            // No local live: record/advance the tombstone.
            (None, _) => writers::upsert_tombstone(tx, bucket, &cid, &row.hlc)?,
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings — LWW per whitelisted key
// ---------------------------------------------------------------------------

fn merge_settings(tx: &Connection, rows: &[ParsedRow]) -> Result<()> {
    for row in rows {
        let key = row
            .value
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CrateError::CloudSync("settings row missing key".into()))?;
        if !crate::services::cloud_sync::is_synced_setting(key) {
            continue; // never apply a non-whitelisted key
        }
        let value = row
            .value
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let local_hlc: String = tx
            .query_row(
                "SELECT value FROM sync_state WHERE key = ?1",
                [format!("setting_hlc:{key}")],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default();
        if hlc_gt(&row.hlc, &local_hlc) {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            tx.execute(
                "INSERT INTO sync_state (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![format!("setting_hlc:{key}"), row.hlc],
            )?;
        }
    }
    Ok(())
}
