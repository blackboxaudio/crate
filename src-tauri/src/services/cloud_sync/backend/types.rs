//! Plain owned data types that cross the [`CloudBackend`](super::CloudBackend)
//! boundary. NO vendor types (no Firestore, no reqwest) — the Phase 1 mock and the
//! Phase 2 Firebase backend share these verbatim, so nothing above the trait knows
//! who the backend is.

use std::collections::BTreeMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Wire-format version of the manifest document. Bump on a breaking shape change.
pub const MANIFEST_FORMAT_VERSION: u32 = 1;

/// One bucket's entry in the manifest. The blob hash is the identity used for
/// diffing; `count`/`hlc` are diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BucketEntry {
    /// blake3 hex of the bucket's serialized (uncompressed) JSONL.
    pub blob_hash: String,
    /// Storage object key the bytes live under (Phase 2 gzips; Phase 1 stores raw).
    pub object_key: String,
    /// Live row count — diagnostic only, NOT part of identity.
    pub count: u64,
    /// Max `_hlc` across the bucket's rows + tombstones (`""` if empty).
    pub hlc: String,
}

/// The vault manifest: the authoritative index of every bucket's current blob.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    /// device_id of whoever last wrote the manifest (self-echo / diagnostics).
    pub last_writer_device: String,
    /// Max bucket hlc across the whole manifest — a monotonic-ish watermark.
    pub manifest_hlc: String,
    /// `Bucket::as_str()` → entry. `BTreeMap` keeps serialization deterministic.
    pub buckets: BTreeMap<String, BucketEntry>,
}

impl Manifest {
    pub fn empty(device_id: &str) -> Self {
        Self {
            format_version: MANIFEST_FORMAT_VERSION,
            last_writer_device: device_id.to_string(),
            manifest_hlc: String::new(),
            buckets: BTreeMap::new(),
        }
    }

    pub fn bucket(&self, name: &str) -> Option<&BucketEntry> {
        self.buckets.get(name)
    }
}

/// Opaque compare-and-swap token. The mock uses a monotonic counter; Firestore
/// (Phase 2) uses the document's server `updateTime` as a write precondition.
/// Callers treat it as opaque: read one, pass it back to `write` as `expected`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEtag(pub(crate) EtagInner);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum EtagInner {
    /// Mock backend: a monotonically increasing version counter.
    Counter(u64),
    /// Server backend: an opaque token (e.g. Firestore `updateTime`).
    Token(String),
}

impl ManifestEtag {
    pub(crate) fn counter(n: u64) -> Self {
        Self(EtagInner::Counter(n))
    }
    pub fn token(s: impl Into<String>) -> Self {
        Self(EtagInner::Token(s.into()))
    }

    /// The opaque server token, if this etag is a server token (Firestore
    /// `updateTime`). Returns `None` for the mock's counter variant.
    pub fn as_token(&self) -> Option<&str> {
        match &self.0 {
            EtagInner::Token(s) => Some(s),
            EtagInner::Counter(_) => None,
        }
    }
}

/// A blob scheduled for deletion once its manifest entry is superseded. Enqueued
/// atomically with the manifest write that orphaned it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcEntry {
    pub object_key: String,
    pub delete_after: SystemTime,
}

/// Server-assigned id for a queued [`GcEntry`] (opaque; mock uses a counter).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GcEntryId(pub String);

/// A device registered against the user's sync account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub device_id: String,
    pub name: String,
    pub last_seen: SystemTime,
    pub app_version: String,
}

/// An authenticated backend session. `access_token_expires_at` drives refresh.
/// Deliberately does NOT derive `PartialEq` — tokens are secrets, not compared.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthSession {
    pub uid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: SystemTime,
    pub email: Option<String>,
    pub display_name: Option<String>,
}
