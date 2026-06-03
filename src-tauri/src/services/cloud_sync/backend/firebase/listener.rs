//! Firestore real-time listener → `BoxStream` of manifest updates.
//!
//! This is a stub that yields no events, so [`ManifestStore::subscribe`] is satisfied
//! without live network plumbing. Phase 3 deliberately drives pull via **runtime
//! polling** rather than a live stream: the orchestrator periodically reads the
//! manifest and re-merges on change (see `runtime::CloudSyncState::run_pull` and
//! `pipeline::pull`). Polling stays backend-agnostic and sidesteps refreshing the
//! ~hourly access token inside a long-lived gRPC `Listen` stream. The `subscribe` trait
//! method is kept as the seam so a true Firestore `Listen` stream can replace polling
//! later — implemented here in place of [`empty_stream`] — without touching the
//! orchestrator.
//!
//! [`ManifestStore::subscribe`]: crate::services::cloud_sync::backend::ManifestStore::subscribe

use futures::stream::{self, BoxStream};

use crate::services::cloud_sync::backend::types::{Manifest, ManifestEtag};

/// A manifest-update stream that never yields (the Phase 3 runtime polls instead).
pub(crate) fn empty_stream() -> BoxStream<'static, (Manifest, ManifestEtag)> {
    Box::pin(stream::empty())
}
