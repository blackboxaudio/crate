//! The production pull pipeline.
//!
//! Brings *other* devices' changes down: read the remote manifest, skip our own
//! echoes, diff it against the local manifest, download the buckets that differ, and
//! feed each through the [`merge_bucket`] engine (HLC + tombstones + add-wins). The DB
//! `MutexGuard` is never held across an `.await` — downloads happen without the guard,
//! and each merge re-takes it.
//!
//! The orchestrator ([`crate::services::cloud_sync::runtime`]) drives [`pull`] on a
//! poll (Phase 3 uses runtime polling rather than a live listener — see
//! `backend/firebase/listener.rs`). A cheap **etag gate** short-circuits idle polls so
//! we only recompute the local manifest when the remote was actually rewritten.
//!
//! [`pull_and_merge`] is the shared download+merge core, reused by the push pipeline's
//! pull-then-merge step so there is one implementation of "download the right blobs and
//! merge them" (`push::push`).

use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension};

use crate::error::{CrateError, Result};

use super::super::backend::types::{AuthSession, Manifest};
use super::super::backend::{BlobStore, CloudBackend, ManifestStore};
use super::buckets::Bucket;
use super::manifest::{compute_local_manifest, diff_manifest};
use super::merge::merge_bucket;
use super::rows;

/// `sync_state` key: serialized etag of the manifest we last observed. Every
/// `manifest.write` mints a fresh etag, so an unchanged etag means nothing was written
/// since our last pull — the change-detection gate.
const LAST_SYNCED_ETAG: &str = "last_synced_manifest_etag";
/// `sync_state` key: the manifest HLC we last merged (diagnostics / forward-compat).
const LAST_SYNCED_HLC: &str = "last_synced_manifest_hlc";

/// Pull other devices' changes into the local DB. Reads the remote manifest, skips
/// self-authored writes, merges any remote-ahead buckets, and records the synced
/// manifest etag/hlc in `sync_state`. Returns whether anything merged.
///
/// Idempotent and safe to call repeatedly: the etag gate makes a no-change poll cheap
/// (one manifest read), and re-merging the same blobs is a no-op in the merge engine.
pub async fn pull(
    conn: Arc<Mutex<Connection>>,
    backend: &Arc<dyn CloudBackend>,
    session: &AuthSession,
    self_device_id: &str,
) -> Result<bool> {
    let store = backend.manifest();
    let blobs = backend.blobs();

    let Some((remote, etag)) = store.read(session).await? else {
        return Ok(false); // no remote vault yet — nothing to pull
    };
    let etag_str = serde_json::to_string(&etag)
        .map_err(|e| CrateError::CloudSync(format!("serialize manifest etag: {e}")))?;

    // Change-detection gate: the manifest doc hasn't been rewritten since our last
    // pull (every write mints a fresh etag), so there is nothing new to merge. This
    // keeps an idle poll to a single read — no local-manifest recompute.
    {
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        if read_sync_state(&guard, LAST_SYNCED_ETAG)?.as_deref() == Some(etag_str.as_str()) {
            return Ok(false);
        }
    }

    // Self-echo: we wrote this manifest. Record the etag so the gate above
    // short-circuits next poll, but don't merge our own state back (wasteful churn).
    if remote.last_writer_device == self_device_id {
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        write_sync_state(&guard, LAST_SYNCED_ETAG, &etag_str)?;
        write_sync_state(&guard, LAST_SYNCED_HLC, &remote.manifest_hlc)?;
        return Ok(false);
    }

    let merged = pull_and_merge(&conn, &blobs, session, &remote).await?;

    let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
    write_sync_state(&guard, LAST_SYNCED_ETAG, &etag_str)?;
    write_sync_state(&guard, LAST_SYNCED_HLC, &remote.manifest_hlc)?;
    Ok(merged)
}

/// Download + merge every bucket the remote has that differs from local, parents
/// before children (so a junction's endpoints already exist). Downloads happen without
/// the guard; each merge re-takes it. Returns `true` if at least one bucket merged.
///
/// Shared by [`pull`] and the push pipeline's pull-then-merge step — the single
/// implementation of "download the right blobs and feed them to the merge engine".
pub async fn pull_and_merge(
    conn: &Arc<Mutex<Connection>>,
    blobs: &Arc<dyn BlobStore>,
    session: &AuthSession,
    remote: &Manifest,
) -> Result<bool> {
    let mut to_download = {
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        // device_id is irrelevant here: diffing keys on blob_hash only.
        let local = compute_local_manifest(&guard, "")?;
        diff_manifest(&local, remote).to_download
    };
    if to_download.is_empty() {
        return Ok(false);
    }

    // Merge parents before children so a junction's endpoints already exist.
    let order = Bucket::merge_order();
    to_download.sort_by_key(|name| {
        order
            .iter()
            .position(|b| b.as_str() == *name)
            .unwrap_or(usize::MAX)
    });

    for name in to_download {
        let bucket = Bucket::parse(&name)
            .ok_or_else(|| CrateError::CloudSync(format!("bad bucket {name}")))?;
        let Some(entry) = remote.bucket(&name) else {
            continue;
        };
        // BucketEntry.object_key is relative; prepend the per-user vault prefix.
        let key = format!("users/{}/vault/{}", session.uid, entry.object_key);
        let bytes = blobs.download(session, &key).await?;
        let parsed = rows::parse_bucket(&bucket, &bytes)?;
        let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        merge_bucket(&guard, &bucket, &parsed)?;
    }
    Ok(true)
}

// --- sync_state helpers -------------------------------------------------------

/// Read a `sync_state` value, or `None` if the key is absent.
fn read_sync_state(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get::<_, String>(0)
        })
        .optional()?)
}

/// Upsert a `sync_state` value.
fn write_sync_state(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}
