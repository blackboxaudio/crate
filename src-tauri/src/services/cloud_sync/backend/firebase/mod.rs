//! The Firebase implementation of [`CloudBackend`](super::CloudBackend).
//!
//! This module and its children are the ONLY place in the app that names Firebase /
//! Google REST APIs. Everything else depends on the vendor-agnostic trait surface in
//! [`super`]; swapping backends is a new sibling here, not a refactor.

use std::sync::Arc;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::AuthSession;
use crate::services::cloud_sync::config::CloudConfig;

use super::{AuthBackend, BlobStore, CloudBackend, DeviceRegistry, ManifestStore};

use appcheck::AppCheckState;

mod appcheck;
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
    /// Mobile App Check token cache (#139). `None` on desktop / when unconfigured, in which case
    /// [`FirebaseInner::authed`] attaches no `X-Firebase-AppCheck` header — unchanged behavior.
    appcheck: Option<Arc<AppCheckState>>,
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

    /// The current App Check token, or `None` when App Check is inactive (desktop/unconfigured)
    /// **or** a mint fails. A mint failure is logged and treated as "no header": during the
    /// monitoring-mode rollout App Check must never break a sync, and once enforcement is enabled
    /// server-side a missing header surfaces naturally as a 403 at the call site.
    async fn appcheck_token(&self) -> Option<String> {
        let state = self.appcheck.as_ref()?;
        match state.ensure_fresh().await {
            Ok(token) => Some(token),
            Err(e) => {
                log::warn!("cloud_sync: App Check token unavailable ({e}); sending request without it");
                None
            }
        }
    }

    /// Build a Firestore/Storage request pre-loaded with the user's bearer auth and, when App
    /// Check is active (mobile), the `X-Firebase-AppCheck` header. The single choke point for
    /// outbound authenticated calls — every existing call site routes through here, so any future
    /// one carries App Check by construction.
    pub(crate) async fn authed(
        &self,
        method: reqwest::Method,
        url: &str,
        session: &AuthSession,
    ) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .request(method, url)
            .bearer_auth(&session.access_token);
        if let Some(token) = self.appcheck_token().await {
            req = req.header("X-Firebase-AppCheck", token);
        }
        req
    }

    /// Attach only the App Check header to a pre-built request. For the Identity-Toolkit /
    /// secure-token calls, which authenticate by `?key=` API key (not bearer) and run before an
    /// [`AuthSession`] exists, so they can't go through [`FirebaseInner::authed`].
    pub(crate) async fn with_appcheck(
        &self,
        req: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        match self.appcheck_token().await {
            Some(token) => req.header("X-Firebase-AppCheck", token),
            None => req,
        }
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
        // Mobile-only: `None` on desktop / when no Firebase App ID is configured (#139).
        let appcheck = appcheck::for_platform(client.clone(), config).map(AppCheckState::new);
        Ok(Self {
            inner: Arc::new(FirebaseInner {
                client,
                config: config.clone(),
                appcheck: appcheck.map(Arc::new),
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
