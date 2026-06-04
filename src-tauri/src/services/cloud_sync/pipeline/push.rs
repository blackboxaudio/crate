//! The production push pipeline.
//!
//! Mirrors the test blueprint (`tests/mod.rs::push`) but with the real lock
//! discipline (the DB `MutexGuard` is never held across an `.await`), CAS retry with
//! backoff, GC enqueue of superseded blobs, and (via the Firebase `BlobStore`) gzip.
//!
//! Flow: stamp any unstamped rows once → CAS loop: read remote manifest → pull-then-
//! merge any remote-ahead buckets via [`pull::pull_and_merge`] (so our write carries
//! the union) → recompute the local manifest → upload changed buckets →
//! `manifest.write` with the prior object keys enqueued for GC → clear the dirty queue
//! on success. The standalone pull, self-echo skip, and live updates live in the
//! sibling [`pull`] module; this push is idempotent and safe to call repeatedly.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use rusqlite::Connection;

use crate::error::{CrateError, Result};

use super::super::backend::types::{AuthSession, GcEntry, Manifest};
use super::super::backend::CloudBackend;
use super::super::hlc;
use super::buckets::Bucket;
use super::dirty::stamp_unstamped_rows;
use super::manifest::{compute_local_manifest, diff_manifest};
use super::merge::OverrideEvent;
use super::pull;
use super::rows;

/// Max CAS attempts before giving up (each retry re-reads + re-merges the remote).
const MAX_RETRIES: usize = 8;
/// Grace period before a superseded blob becomes eligible for GC (Phase 3 sweep).
const GC_GRACE: Duration = Duration::from_secs(3600);

/// The result of a push: any non-trivial overrides observed while pull-then-merging the
/// remote (for the override toast), plus the plain names of the buckets that pull-then-
/// merge brought down from peers (so the runtime can tell the UI to reload them — a push
/// merges the remote union before uploading, so "Sync now" must refresh too).
pub struct PushOutcome {
    pub overrides: Vec<OverrideEvent>,
    pub merged_buckets: Vec<String>,
}

/// Push all local changes to the cloud. Safe to call repeatedly; clears
/// `sync_dirty_buckets` only on a successful manifest commit. Returns any non-trivial
/// overrides observed while pull-then-merging the remote, plus the buckets merged from
/// peers during that step.
pub async fn push(
    conn: Arc<Mutex<Connection>>,
    backend: &Arc<dyn CloudBackend>,
    session: &AuthSession,
    device_id: &str,
) -> Result<PushOutcome> {
    // One-time first-sync stamping (internally gated by `initial_stamp_done`).
    {
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let node_id = hlc::load_node_id(&guard)?;
        stamp_unstamped_rows(&guard, node_id)?;
    }

    let store = backend.manifest();
    let blobs = backend.blobs();
    let mut backoff = Duration::from_millis(200);
    let mut overrides = Vec::new();
    let mut merged_buckets = Vec::new();

    for attempt in 0..MAX_RETRIES {
        let (remote_manifest, expected) = match store.read(session).await? {
            Some((m, e)) => (Some(m), Some(e)),
            None => (None, None),
        };

        // Pull-then-merge remote-ahead buckets so our write carries the union. A re-merge
        // of already-applied rows is a no-op, so retries don't double-count overrides (the
        // runtime dedupes the bucket names before signaling the UI).
        if let Some(rm) = &remote_manifest {
            let outcome = pull::pull_and_merge(&conn, &blobs, session, rm).await?;
            overrides.extend(outcome.overrides);
            merged_buckets.extend(outcome.buckets);
        }

        // Recompute the local manifest + serialize the changed buckets (all sync work
        // happens under the guard; the guard is dropped before any upload). This also
        // snapshots the dirty rows this attempt claims (see `PreparedPush`).
        let prepared = prepare_uploads(&conn, session, device_id, remote_manifest.as_ref())?;

        for (key, bytes) in &prepared.uploads {
            blobs
                .upload(
                    session,
                    key,
                    Bytes::from(bytes.clone()),
                    "application/x-ndjson",
                )
                .await?;
        }

        match store
            .write(
                session,
                &prepared.manifest,
                expected.as_ref(),
                &prepared.gc_enqueue,
            )
            .await
        {
            Ok(_) => {
                // Clear ONLY the rows this snapshot claimed. A mutation that marked a
                // bucket dirty during the push has a newer `(bucket, marked_at)` pair
                // that is absent here, so it survives to trigger the next push instead
                // of being silently wiped.
                let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
                for (bucket, marked_at) in &prepared.claimed_dirty {
                    guard.execute(
                        "DELETE FROM sync_dirty_buckets WHERE bucket = ?1 AND marked_at = ?2",
                        rusqlite::params![bucket, marked_at],
                    )?;
                }
                return Ok(PushOutcome {
                    overrides,
                    merged_buckets,
                });
            }
            Err(CrateError::CloudSyncConflict) => {
                log::info!(
                    "cloud_sync: push CAS conflict (attempt {}/{MAX_RETRIES}), retrying",
                    attempt + 1
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(3));
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Err(CrateError::CloudSync("push exceeded CAS retries".into()))
}

/// A snapshot taken for one push attempt: the manifest to write, the blobs to upload,
/// the prior object keys to enqueue for GC, and the exact dirty rows this snapshot
/// claims. The claimed rows are deleted only on a successful commit, so a mutation that
/// marks a bucket dirty *during* the push (after this snapshot) keeps its trigger and
/// drives the next push rather than being silently cleared.
struct PreparedPush {
    manifest: Manifest,
    uploads: Vec<(String, Vec<u8>)>,
    gc_enqueue: Vec<GcEntry>,
    claimed_dirty: Vec<(String, String)>,
}

/// Recompute the local manifest and serialize every bucket that differs from `remote`.
/// Synchronous — holds the guard for its whole body, so the dirty-row snapshot is taken
/// atomically with the data snapshot (the correctness crux of the coalescing fix).
fn prepare_uploads(
    conn: &Arc<Mutex<Connection>>,
    session: &AuthSession,
    device_id: &str,
    remote: Option<&Manifest>,
) -> Result<PreparedPush> {
    let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
    let local = compute_local_manifest(&guard, device_id)?;
    let base = remote
        .cloned()
        .unwrap_or_else(|| Manifest::empty(device_id));
    let diff = diff_manifest(&local, &base);

    let mut uploads = Vec::new();
    let mut gc_enqueue = Vec::new();
    let delete_after = SystemTime::now() + GC_GRACE;

    for name in &diff.to_upload {
        let bucket = Bucket::parse(name)
            .ok_or_else(|| CrateError::CloudSync(format!("bad bucket {name}")))?;
        let bytes = rows::serialize_bucket(&guard, &bucket)?;
        let hash = rows::bucket_hash(&bytes);
        // Full storage key (BucketEntry.object_key is relative — see manifest.rs).
        let key = format!(
            "users/{}/vault/{}-{}.jsonl.gz",
            session.uid,
            bucket.as_str(),
            hash
        );
        uploads.push((key, bytes));

        // Enqueue the prior blob for GC when this bucket actually changed key.
        if let (Some(prev), Some(now)) = (
            base.bucket(name).filter(|_| remote.is_some()),
            local.bucket(name),
        ) {
            if prev.object_key != now.object_key {
                gc_enqueue.push(GcEntry {
                    object_key: format!("users/{}/vault/{}", session.uid, prev.object_key),
                    delete_after,
                });
            }
        }
    }

    // Snapshot the exact dirty rows this push claims, atomically with the data above
    // (same guard). On success we delete ONLY these `(bucket, marked_at)` pairs, so a
    // concurrent mutation's newer mark survives and triggers the next push.
    let claimed_dirty = {
        let mut stmt = guard.prepare("SELECT bucket, marked_at FROM sync_dirty_buckets")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<(String, String)>, _>>()?;
        rows
    };

    Ok(PreparedPush {
        manifest: local,
        uploads,
        gc_enqueue,
        claimed_dirty,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    fn dirty_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sync_dirty_buckets (bucket TEXT PRIMARY KEY, marked_at TEXT NOT NULL);",
        )
        .unwrap();
        conn
    }

    /// Mirrors `dirty::mark_dirty`.
    fn mark(conn: &Connection, bucket: &str, at: &str) {
        conn.execute(
            "INSERT INTO sync_dirty_buckets (bucket, marked_at) VALUES (?1, ?2)
             ON CONFLICT(bucket) DO UPDATE SET marked_at = excluded.marked_at",
            params![bucket, at],
        )
        .unwrap();
    }

    /// Mirrors the snapshot in `prepare_uploads`.
    fn claim(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT bucket, marked_at FROM sync_dirty_buckets")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<(String, String)>, _>>()
            .unwrap()
    }

    /// Mirrors the post-commit clear in `push`.
    fn clear_claimed(conn: &Connection, claimed: &[(String, String)]) {
        for (bucket, marked_at) in claimed {
            conn.execute(
                "DELETE FROM sync_dirty_buckets WHERE bucket = ?1 AND marked_at = ?2",
                params![bucket, marked_at],
            )
            .unwrap();
        }
    }

    fn buckets(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT bucket FROM sync_dirty_buckets ORDER BY bucket")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<String>, _>>()
            .unwrap()
    }

    /// A bucket marked dirty (or re-marked) *during* a push must survive the post-commit
    /// clear: only the exact rows claimed at snapshot time are deleted, so the newer
    /// mark keeps its trigger and drives the next push (the coalescing-race fix).
    #[test]
    fn concurrent_dirty_marks_survive_the_clear() {
        let conn = dirty_db();
        // Two buckets dirty when the push snapshots them.
        mark(&conn, "playlists", "2026-06-03T00:00:00.000000001+00:00");
        mark(&conn, "tags", "2026-06-03T00:00:00.000000002+00:00");
        let claimed = claim(&conn);

        // During the upload/CAS window: "playlists" is edited again (marked_at bumped via
        // ON CONFLICT) and a brand-new "tracks/3" bucket is dirtied.
        mark(&conn, "playlists", "2026-06-03T00:00:05.000000000+00:00");
        mark(&conn, "tracks/3", "2026-06-03T00:00:05.000000000+00:00");

        clear_claimed(&conn, &claimed);

        // "tags" was synced and untouched → cleared. "playlists" (re-marked) and
        // "tracks/3" (new) survive to trigger the next push.
        assert_eq!(
            buckets(&conn),
            vec!["playlists".to_string(), "tracks/3".to_string()]
        );
    }

    /// A clean push with no concurrent mutation clears the whole queue.
    #[test]
    fn quiescent_push_clears_all_claimed() {
        let conn = dirty_db();
        mark(&conn, "playlists", "2026-06-03T00:00:00.000000001+00:00");
        mark(&conn, "tags", "2026-06-03T00:00:00.000000002+00:00");
        let claimed = claim(&conn);
        clear_claimed(&conn, &claimed);
        assert!(buckets(&conn).is_empty());
    }
}
