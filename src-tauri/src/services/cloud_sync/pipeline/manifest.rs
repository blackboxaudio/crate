//! Local manifest computation and diffing — backend-agnostic (operates on the
//! plain [`Manifest`] type, never on Firestore).
//!
//! **INCLUDE-ALL policy:** every bucket in `synced_buckets()` gets an entry, even when
//! empty. An empty bucket serializes to zero bytes, whose blake3 is a fixed value, so two
//! devices' empty buckets share a hash and never spuriously diff. Diffing keys on
//! `blob_hash` only (`count`/`hlc` are diagnostics). A scoped node (mobile) omits the
//! library buckets here and relies on [`preserve_unmanaged`] so its manifest write still
//! carries a peer's library entries forward instead of dropping them.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;

use crate::error::Result;

use super::super::backend::types::{BucketEntry, Manifest, MANIFEST_FORMAT_VERSION};
use super::buckets::{self, Bucket};
use super::rows;

/// The Cloud Storage object key a bucket blob lives under. Phase 1 mock stores raw
/// JSONL under the same key (the `.jsonl.gz` suffix is kept so it matches Phase 2).
pub fn object_key(uid: &str, bucket: &Bucket, blob_hash: &str) -> String {
    format!(
        "users/{uid}/vault/{}-{}.jsonl.gz",
        bucket.as_str(),
        blob_hash
    )
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ManifestDiff {
    /// Bucket names whose local blob differs from (or is missing on) the remote.
    pub to_upload: Vec<String>,
    /// Bucket names whose remote blob differs from (or is missing on) local.
    pub to_download: Vec<String>,
}

/// Compute this device's manifest over the buckets this node syncs (every bucket on
/// desktop; the discovery-only subset on mobile — see [`buckets::synced_buckets`]).
pub fn compute_local_manifest(conn: &Connection, device_id: &str) -> Result<Manifest> {
    let mut buckets_map = BTreeMap::new();
    let mut manifest_hlc = String::new();
    for bucket in buckets::synced_buckets() {
        let bytes = rows::serialize_bucket(conn, &bucket)?;
        let blob_hash = rows::bucket_hash(&bytes);
        let count = count_live_rows(conn, &bucket)?;
        let hlc = rows::bucket_max_hlc(conn, &bucket)?;
        if hlc > manifest_hlc {
            manifest_hlc = hlc.clone();
        }
        buckets_map.insert(
            bucket.as_str(),
            BucketEntry {
                object_key: format!("{}-{}.jsonl.gz", bucket.as_str(), blob_hash),
                blob_hash,
                count,
                hlc,
            },
        );
    }
    Ok(Manifest {
        format_version: MANIFEST_FORMAT_VERSION,
        last_writer_device: device_id.to_string(),
        manifest_hlc,
        buckets: buckets_map,
    })
}

/// Which buckets must be uploaded (local differs) / downloaded (remote differs).
pub fn diff_manifest(local: &Manifest, remote: &Manifest) -> ManifestDiff {
    let mut diff = ManifestDiff::default();
    let mut names: BTreeSet<&String> = local.buckets.keys().collect();
    names.extend(remote.buckets.keys());
    for name in names {
        let l = local.buckets.get(name).map(|e| &e.blob_hash);
        let r = remote.buckets.get(name).map(|e| &e.blob_hash);
        match (l, r) {
            (Some(lh), Some(rh)) if lh == rh => {}
            (Some(_), Some(_)) => {
                diff.to_upload.push(name.clone());
                diff.to_download.push(name.clone());
            }
            (Some(_), None) => diff.to_upload.push(name.clone()),
            (None, Some(_)) => diff.to_download.push(name.clone()),
            (None, None) => {}
        }
    }
    diff
}

/// Carry forward a peer's entries for buckets this node does not manage.
///
/// A **scoped** node (a discovery-only mobile build — see [`buckets::synced_buckets`])
/// computes a local manifest that omits the library buckets entirely. The remote
/// manifest is the **shared** per-user document, so writing a scoped manifest verbatim
/// would drop those library entries and orphan their blobs. For every `remote` bucket
/// absent from `local`, copy the remote entry in unchanged; `local` always wins for the
/// buckets it manages. On desktop (full scope) `local` already has every bucket, so this
/// is a no-op.
pub fn preserve_unmanaged(mut local: Manifest, remote: Option<&Manifest>) -> Manifest {
    if let Some(remote) = remote {
        for (name, entry) in &remote.buckets {
            local
                .buckets
                .entry(name.clone())
                .or_insert_with(|| entry.clone());
        }
    }
    local
}

fn count_live_rows(conn: &Connection, bucket: &Bucket) -> Result<u64> {
    match bucket {
        Bucket::Settings => {
            let mut n = 0u64;
            for key in crate::services::cloud_sync::SYNCED_SETTING_KEYS {
                let present: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM settings WHERE key = ?1)",
                    [key],
                    |r| r.get(0),
                )?;
                if present {
                    n += 1;
                }
            }
            Ok(n)
        }
        Bucket::Tracks(shard) => {
            let mut count = 0u64;
            let mut stmt = conn.prepare("SELECT id FROM tracks")?;
            let ids = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for id in ids {
                if buckets::shard_for_track_id(&id?) == *shard {
                    count += 1;
                }
            }
            Ok(count)
        }
        _ => {
            let table = bucket.table();
            let c: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            Ok(c as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cloud_sync::backend::types::BucketEntry;

    fn manifest_with(entries: &[(&str, &str)]) -> Manifest {
        let mut m = Manifest::empty("dev");
        for (name, hash) in entries {
            m.buckets.insert(
                name.to_string(),
                BucketEntry {
                    blob_hash: hash.to_string(),
                    object_key: format!("{name}-{hash}.jsonl.gz"),
                    count: 0,
                    hlc: String::new(),
                },
            );
        }
        m
    }

    #[test]
    fn identical_manifests_have_no_diff() {
        let a = manifest_with(&[("playlists", "h1"), ("tags", "h2")]);
        let b = manifest_with(&[("playlists", "h1"), ("tags", "h2")]);
        assert_eq!(diff_manifest(&a, &b), ManifestDiff::default());
    }

    #[test]
    fn differing_hash_uploads_and_downloads() {
        let local = manifest_with(&[("playlists", "h1")]);
        let remote = manifest_with(&[("playlists", "h2")]);
        let d = diff_manifest(&local, &remote);
        assert_eq!(d.to_upload, vec!["playlists".to_string()]);
        assert_eq!(d.to_download, vec!["playlists".to_string()]);
    }

    #[test]
    fn one_sided_buckets_go_the_right_way() {
        let local = manifest_with(&[("playlists", "h1")]);
        let remote = manifest_with(&[("tags", "h2")]);
        let d = diff_manifest(&local, &remote);
        assert_eq!(d.to_upload, vec!["playlists".to_string()]);
        assert_eq!(d.to_download, vec!["tags".to_string()]);
    }

    #[test]
    fn preserve_unmanaged_carries_forward_remote_only_buckets() {
        // A scoped (mobile-like) local manifest holds only a discovery bucket.
        let local = manifest_with(&[("discovery_releases", "d1")]);
        // The shared remote also has a library bucket this node does not manage.
        let remote = manifest_with(&[("discovery_releases", "d0"), ("tracks/3", "t9")]);

        let merged = preserve_unmanaged(local, Some(&remote));

        // Local wins for the bucket it manages...
        assert_eq!(merged.buckets["discovery_releases"].blob_hash, "d1");
        // ...and the unmanaged remote bucket is carried forward unchanged (no orphan).
        assert_eq!(merged.buckets["tracks/3"].blob_hash, "t9");
        assert_eq!(merged.buckets.len(), 2);
    }

    #[test]
    fn preserve_unmanaged_without_remote_is_noop() {
        let local = manifest_with(&[("discovery_releases", "d1")]);
        let merged = preserve_unmanaged(local, None);
        assert_eq!(merged.buckets.len(), 1);
        assert_eq!(merged.buckets["discovery_releases"].blob_hash, "d1");
    }
}
