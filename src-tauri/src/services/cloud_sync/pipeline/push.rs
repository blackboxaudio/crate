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
use super::pull;
use super::rows;

/// Max CAS attempts before giving up (each retry re-reads + re-merges the remote).
const MAX_RETRIES: usize = 8;
/// Grace period before a superseded blob becomes eligible for GC (Phase 3 sweep).
const GC_GRACE: Duration = Duration::from_secs(3600);

/// Push all local changes to the cloud. Safe to call repeatedly; clears
/// `sync_dirty_buckets` only on a successful manifest commit.
pub async fn push(
    conn: Arc<Mutex<Connection>>,
    backend: &Arc<dyn CloudBackend>,
    session: &AuthSession,
    device_id: &str,
) -> Result<()> {
    // One-time first-sync stamping (internally gated by `initial_stamp_done`).
    {
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let node_id = hlc::load_node_id(&guard)?;
        stamp_unstamped_rows(&guard, node_id)?;
    }

    let store = backend.manifest();
    let blobs = backend.blobs();
    let mut backoff = Duration::from_millis(200);

    for attempt in 0..MAX_RETRIES {
        let (remote_manifest, expected) = match store.read(session).await? {
            Some((m, e)) => (Some(m), Some(e)),
            None => (None, None),
        };

        // Pull-then-merge remote-ahead buckets so our write carries the union.
        if let Some(rm) = &remote_manifest {
            pull::pull_and_merge(&conn, &blobs, session, rm).await?;
        }

        // Recompute the local manifest + serialize the changed buckets (all sync work
        // happens under the guard; the guard is dropped before any upload).
        let (local, uploads, gc_enqueue) =
            prepare_uploads(&conn, session, device_id, remote_manifest.as_ref())?;

        for (key, bytes) in &uploads {
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
            .write(session, &local, expected.as_ref(), &gc_enqueue)
            .await
        {
            Ok(_) => {
                let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
                guard.execute("DELETE FROM sync_dirty_buckets", [])?;
                return Ok(());
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

/// Recompute the local manifest and serialize every bucket that differs from
/// `remote`, returning the manifest, the `(full_key, bytes)` uploads, and the prior
/// object keys to enqueue for GC. Synchronous — holds the guard for its whole body.
#[allow(clippy::type_complexity)]
fn prepare_uploads(
    conn: &Arc<Mutex<Connection>>,
    session: &AuthSession,
    device_id: &str,
    remote: Option<&Manifest>,
) -> Result<(Manifest, Vec<(String, Vec<u8>)>, Vec<GcEntry>)> {
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
        let bucket =
            Bucket::parse(name).ok_or_else(|| CrateError::CloudSync(format!("bad bucket {name}")))?;
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

    Ok((local, uploads, gc_enqueue))
}
