//! Firestore real-time listener → `BoxStream` of manifest updates.
//!
//! Phase 2 ships a stub that yields no events, so [`ManifestStore::subscribe`] is
//! satisfied without live network plumbing. Phase 3 replaces [`empty_stream`] with the
//! real Firestore `Listen` stream that drives the pull pipeline.
//!
//! [`ManifestStore::subscribe`]: crate::services::cloud_sync::backend::ManifestStore::subscribe

use futures::stream::{self, BoxStream};

use crate::services::cloud_sync::backend::types::{Manifest, ManifestEtag};

/// A manifest-update stream that never yields (Phase 2 placeholder).
pub(crate) fn empty_stream() -> BoxStream<'static, (Manifest, ManifestEtag)> {
    Box::pin(stream::empty())
}
