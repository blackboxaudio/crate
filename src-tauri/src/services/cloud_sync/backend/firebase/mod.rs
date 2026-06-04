//! The Firebase implementation of [`CloudBackend`](super::CloudBackend).
//!
//! This module and its children are the ONLY place in the app that names Firebase /
//! Google REST APIs. Everything else depends on the vendor-agnostic trait surface in
//! [`super`]; swapping backends is a new sibling here, not a refactor.

use std::sync::Arc;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::config::CloudConfig;

use super::{AuthBackend, BlobStore, CloudBackend, DeviceRegistry, ManifestStore};

mod auth;
mod blobs;
mod devices;
mod listener;
mod manifest;
mod rest;

/// Shared HTTP client + config handed to each facet (one allocation, cloned `Arc`s).
pub(crate) struct FirebaseInner {
    pub(crate) client: reqwest::Client,
    pub(crate) config: CloudConfig,
}

impl FirebaseInner {
    /// Firestore document base: `.../databases/(default)/documents`.
    pub(crate) fn firestore_base(&self) -> String {
        format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents",
            self.config.project_id
        )
    }

    /// Fully-qualified Firestore document resource name (for `:commit` writes), e.g.
    /// `projects/{pid}/databases/(default)/documents/users/{uid}/...`.
    pub(crate) fn doc_name(&self, relative: &str) -> String {
        format!(
            "projects/{}/databases/(default)/documents/{relative}",
            self.config.project_id
        )
    }

    /// Firebase Storage object base for the configured bucket.
    pub(crate) fn storage_base(&self) -> String {
        format!(
            "https://firebasestorage.googleapis.com/v0/b/{}/o",
            self.config.storage_bucket
        )
    }
}

/// Firebase backend root. Holds the shared client/config and hands out the four
/// trait facets, each a thin wrapper over a cloned `Arc<FirebaseInner>`.
pub struct FirebaseBackend {
    inner: Arc<FirebaseInner>,
}

impl FirebaseBackend {
    pub fn new(config: &CloudConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| CrateError::CloudSync(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            inner: Arc::new(FirebaseInner {
                client,
                config: config.clone(),
            }),
        })
    }
}

impl CloudBackend for FirebaseBackend {
    fn id(&self) -> &'static str {
        "firebase"
    }
    fn auth(&self) -> Arc<dyn AuthBackend> {
        Arc::new(auth::FirebaseAuth::new(self.inner.clone()))
    }
    fn manifest(&self) -> Arc<dyn ManifestStore> {
        Arc::new(manifest::FirebaseManifest::new(self.inner.clone()))
    }
    fn blobs(&self) -> Arc<dyn BlobStore> {
        Arc::new(blobs::FirebaseBlobs::new(self.inner.clone()))
    }
    fn devices(&self) -> Arc<dyn DeviceRegistry> {
        Arc::new(devices::FirebaseDevices::new(self.inner.clone()))
    }
}
