//! Convergence test harness. Two in-memory devices share one [`MockCloudBackend`];
//! `push`/`pull` drive the real serialize → manifest-diff → merge pipeline, and
//! `state_hash` is the convergence oracle (blake3 over every synced bucket).
//!
//! These `push`/`pull` helpers are deliberately a SIMPLIFIED, test-only stand-in:
//! the production push (Phase 2) / pull (Phase 3) add auth, gzip, retry/backoff,
//! GC, and progress events. Here they exist only to exercise the merge engine.

use bytes::Bytes;
use rusqlite::Connection;

use crate::db::schema::get_migrations;
use crate::error::Result;
use crate::services::cloud_sync::backend::mock::MockCloudBackend;
use crate::services::cloud_sync::backend::types::{AuthSession, Manifest};
use crate::services::cloud_sync::backend::CloudBackend;
use crate::services::cloud_sync::pipeline::buckets::Bucket;
use crate::services::cloud_sync::pipeline::dirty::stamp_unstamped_rows;
use crate::services::cloud_sync::pipeline::manifest::{compute_local_manifest, diff_manifest};
use crate::services::cloud_sync::pipeline::merge::merge_bucket;
use crate::services::cloud_sync::pipeline::rows;

mod convergence;

/// A fresh in-memory device: migrations applied, FK on, a distinct `node_id`.
fn new_device(node_id: u32) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    for sql in get_migrations() {
        conn.execute_batch(sql).unwrap();
    }
    // node_id is stored as 8 hex chars (how hlc::load_node_id parses it).
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES ('node_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [format!("{node_id:08x}")],
    )
    .unwrap();
    conn
}

fn test_session() -> AuthSession {
    AuthSession {
        uid: "test-uid".into(),
        access_token: "t".into(),
        refresh_token: "t".into(),
        access_token_expires_at: std::time::SystemTime::now(),
        email: None,
        display_name: None,
        photo_url: None,
    }
}

fn read_node_id(conn: &Connection) -> u32 {
    let s: String = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'node_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    u32::from_str_radix(&s, 16).unwrap()
}

fn key_for(uid: &str, bucket: &Bucket, hash: &str) -> String {
    format!("users/{uid}/vault/{}-{}.jsonl.gz", bucket.as_str(), hash)
}

/// The convergence oracle: blake3 over `serialize_bucket` of every synced bucket,
/// in canonical order, each blob length- and name-prefixed so bucket boundaries are
/// unambiguous. Two devices with equal state hash are byte-for-byte converged.
fn state_hash(conn: &Connection) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    for bucket in Bucket::all() {
        let bytes = rows::serialize_bucket(conn, &bucket)?;
        hasher.update(bucket.as_str().as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Simplified push: stamp any unstamped rows, then (with CAS retry) merge the
/// current remote into local, upload changed bucket blobs, and write the full local
/// manifest. Clears the dirty queue on success.
async fn push(conn: &Connection, backend: &MockCloudBackend, device_id: &str) -> Result<()> {
    let s = test_session();
    let store = backend.manifest();
    let blobs = backend.blobs();

    stamp_unstamped_rows(conn, read_node_id(conn))?;

    const MAX_RETRIES: usize = 8;
    for _ in 0..MAX_RETRIES {
        let remote = store.read(&s).await?;
        let (remote_manifest, expected) = match remote {
            Some((m, e)) => (Some(m), Some(e)),
            None => (None, None),
        };

        // Pull-then-merge any remote-ahead buckets before recomputing, so our write
        // carries the union of both sides.
        if let Some(rm) = &remote_manifest {
            pull_into(conn, backend, rm).await?;
        }

        let local = compute_local_manifest(conn, device_id)?;
        let base = remote_manifest.unwrap_or_else(|| Manifest::empty(device_id));
        let diff = diff_manifest(&local, &base);

        for name in &diff.to_upload {
            let bucket = Bucket::parse(name).expect("manifest name is a valid bucket");
            let bytes = rows::serialize_bucket(conn, &bucket)?;
            let hash = rows::bucket_hash(&bytes);
            blobs
                .upload(
                    &s,
                    &key_for(&s.uid, &bucket, &hash),
                    Bytes::from(bytes),
                    "application/x-ndjson",
                )
                .await?;
        }

        match store.write(&s, &local, expected.as_ref(), &[]).await {
            Ok(_) => {
                conn.execute("DELETE FROM sync_dirty_buckets", []).unwrap();
                return Ok(());
            }
            Err(crate::error::CrateError::CloudSyncConflict) => continue, // re-read + retry
            Err(e) => return Err(e),
        }
    }
    Err(crate::error::CrateError::CloudSync(
        "push exceeded CAS retries".into(),
    ))
}

/// Read the remote manifest and merge any differing buckets into local.
async fn pull(conn: &Connection, backend: &MockCloudBackend) -> Result<()> {
    let s = test_session();
    let Some((remote, _etag)) = backend.manifest().read(&s).await? else {
        return Ok(());
    };
    pull_into(conn, backend, &remote).await
}

async fn pull_into(conn: &Connection, backend: &MockCloudBackend, remote: &Manifest) -> Result<()> {
    let s = test_session();
    let blobs = backend.blobs();
    let local = compute_local_manifest(conn, "")?;
    let diff = diff_manifest(&local, remote);

    // Merge parents before children so a junction's endpoints already exist.
    let order = Bucket::merge_order();
    let mut to_download = diff.to_download;
    to_download.sort_by_key(|name| {
        order
            .iter()
            .position(|b| b.as_str() == *name)
            .unwrap_or(usize::MAX)
    });

    for name in to_download {
        let bucket = Bucket::parse(&name).expect("manifest name is a valid bucket");
        let entry = remote
            .bucket(&name)
            .expect("download target present in remote manifest");
        let key = format!("users/{}/vault/{}", s.uid, entry.object_key);
        let bytes = blobs.download(&s, &key).await?;
        let parsed = rows::parse_bucket(&bucket, &bytes)?;
        merge_bucket(conn, &bucket, &parsed)?;
    }
    Ok(())
}
