//! The garbage-collection sweep.
//!
//! When a push replaces a bucket blob, the prior (now-superseded) object key is
//! enqueued into the manifest's GC queue with `delete_after = now + GC_GRACE` (see
//! `push`). This sweep reclaims those blobs once the grace window has passed: dequeue
//! the due entries, delete each blob, ack the queue entry.
//!
//! It runs once per session at startup and is best-effort — a delete of an
//! already-gone blob is logged but still acked, so a failed delete never strands a
//! queue entry.
//!
//! **Live-reference guard:** blob keys are content-addressed, so a key enqueued when a
//! bucket was superseded can be RE-referenced later (the bucket serializing back to
//! byte-identical content re-mints the same key, and pushes only upload buckets whose
//! hash differs — nothing re-creates the blob). Deleting such a key would leave the
//! manifest pointing at a missing blob and wedge every pull. The sweep therefore reads
//! the current manifest first and ACKS (without deleting) any entry it still references
//! — safe because a future push that supersedes the key again re-enqueues it.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::SystemTime;

use crate::error::Result;

use super::super::backend::types::AuthSession;
use super::super::backend::CloudBackend;

/// Entries pulled from the GC queue per round.
const GC_BATCH: usize = 100;
/// Hard backstop on entries processed in one sweep. Guards against a backend that ever
/// fails to remove an acked entry (which would otherwise re-dequeue indefinitely).
const GC_MAX_TOTAL: usize = 10_000;

/// Reclaim superseded blobs whose grace window has elapsed. Returns the number of
/// queue entries processed. Best-effort: a failed blob delete is logged but still
/// acked so the queue drains.
pub async fn gc_sweep(backend: &Arc<dyn CloudBackend>, session: &AuthSession) -> Result<usize> {
    let store = backend.manifest();
    let blobs = backend.blobs();
    let mut processed = 0usize;

    // Full storage keys the current manifest references (BucketEntry.object_key is
    // relative). Read once per sweep: the residual race (a push re-referencing a key
    // between this read and the delete below) is a seconds-wide window against the
    // hour-long GC grace, and the pull pipeline self-heals a dangling reference anyway.
    let live: HashSet<String> = match store.read(session).await? {
        Some((manifest, _)) => manifest
            .buckets
            .values()
            .map(|e| format!("users/{}/vault/{}", session.uid, e.object_key))
            .collect(),
        None => HashSet::new(),
    };

    loop {
        // Re-evaluate `now` each round so entries that come due mid-sweep are caught.
        let due = store
            .dequeue_gc(session, SystemTime::now(), GC_BATCH)
            .await?;
        if due.is_empty() {
            break;
        }
        for (id, entry) in due {
            // A re-referenced key: the manifest points at this blob again. Ack without
            // deleting — a future supersession re-enqueues it.
            if live.contains(&entry.object_key) {
                log::info!(
                    "cloud_sync: gc skipped still-referenced blob {}",
                    entry.object_key.rsplit('/').next().unwrap_or_default()
                );
                store.ack_gc(session, id).await?;
                processed += 1;
                if processed >= GC_MAX_TOTAL {
                    log::warn!("cloud_sync: gc sweep hit cap of {GC_MAX_TOTAL}, stopping early");
                    return Ok(processed);
                }
                continue;
            }
            // Tolerate an already-deleted blob — still ack so the entry doesn't strand.
            if let Err(e) = blobs.delete(session, &entry.object_key).await {
                log::warn!(
                    "cloud_sync: gc blob delete failed ({}): {e}",
                    entry.object_key
                );
            }
            store.ack_gc(session, id).await?;
            processed += 1;
            if processed >= GC_MAX_TOTAL {
                log::warn!("cloud_sync: gc sweep hit cap of {GC_MAX_TOTAL}, stopping early");
                return Ok(processed);
            }
        }
    }
    Ok(processed)
}
