//! The `CloudBackend` trait surface: auth + manifest + blobs + device registry.
//!
//! Phase 1 ships exactly one implementation — [`mock::MockCloudBackend`] — which
//! powers the convergence integration test. Phase 2 adds a `firebase` sibling that
//! implements the same traits; nothing above this boundary names a vendor type.

use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;

use crate::error::Result;

use super::config::CloudConfig;

pub mod firebase;
pub mod mock;
pub mod types;

use types::{AuthSession, DeviceRecord, GcEntry, GcEntryId, Manifest, ManifestEtag};

/// A complete cloud backend: the four facets a sync engine needs.
pub trait CloudBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn auth(&self) -> Arc<dyn AuthBackend>;
    fn manifest(&self) -> Arc<dyn ManifestStore>;
    fn blobs(&self) -> Arc<dyn BlobStore>;
    fn devices(&self) -> Arc<dyn DeviceRegistry>;
}

#[async_trait]
pub trait AuthBackend: Send + Sync {
    /// Exchange an identity-provider ID token for a backend session.
    async fn sign_in_with_idp(&self, provider_id: &str, id_token: &str) -> Result<AuthSession>;
    async fn refresh(&self, refresh_token: &str) -> Result<AuthSession>;
    async fn sign_out(&self, session: &AuthSession) -> Result<()>;
}

#[async_trait]
pub trait ManifestStore: Send + Sync {
    async fn read(&self, session: &AuthSession) -> Result<Option<(Manifest, ManifestEtag)>>;

    /// Compare-and-swap write. `expected == None` means "create only if absent".
    /// A stale/mismatched `expected` MUST return
    /// [`CrateError::CloudSyncConflict`](crate::error::CrateError::CloudSyncConflict).
    /// On success the `gc_enqueue` blobs are queued atomically with the swap.
    async fn write(
        &self,
        session: &AuthSession,
        manifest: &Manifest,
        expected: Option<&ManifestEtag>,
        gc_enqueue: &[GcEntry],
    ) -> Result<ManifestEtag>;

    /// Live manifest updates. Backends without native streams poll. Phase 1 sync is
    /// pull-on-demand; the mock returns an empty stream.
    async fn subscribe(
        &self,
        session: &AuthSession,
    ) -> Result<BoxStream<'static, (Manifest, ManifestEtag)>>;

    async fn dequeue_gc(
        &self,
        session: &AuthSession,
        due_before: SystemTime,
        limit: usize,
    ) -> Result<Vec<(GcEntryId, GcEntry)>>;

    async fn ack_gc(&self, session: &AuthSession, id: GcEntryId) -> Result<()>;
}

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn upload(
        &self,
        session: &AuthSession,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<()>;

    /// MUST return
    /// [`CrateError::CloudSyncBlobNotFound`](crate::error::CrateError::CloudSyncBlobNotFound)
    /// when the key is absent.
    async fn download(&self, session: &AuthSession, key: &str) -> Result<Bytes>;
    async fn delete(&self, session: &AuthSession, key: &str) -> Result<()>;
}

#[async_trait]
pub trait DeviceRegistry: Send + Sync {
    async fn upsert(&self, session: &AuthSession, device: &DeviceRecord) -> Result<()>;
    async fn list(&self, session: &AuthSession) -> Result<Vec<DeviceRecord>>;
    async fn remove(&self, session: &AuthSession, device_id: &str) -> Result<()>;
}

/// The construction seam: build the default cloud backend (Firebase) from config.
///
/// Everything above the trait boundary holds an `Arc<dyn CloudBackend>` and never
/// names a vendor type; this is the one function that does. `CloudConfig` lives in
/// [`super::config`]; loading degrades gracefully (sync is simply unavailable when
/// the config file is absent), so this is only ever called with a complete config.
pub fn build_default_backend(config: &CloudConfig) -> Result<Arc<dyn CloudBackend>> {
    Ok(Arc::new(firebase::FirebaseBackend::new(config)?))
}
