//! In-memory [`CloudBackend`] for tests. A `Clone` shares the same backing store,
//! so two "devices" can talk to one cloud. Mutable state sits behind a single
//! `tokio::sync::Mutex` (the codebase norm; its guard is `Send`, keeping the
//! `#[async_trait]` futures `Send`). No `.await` is ever held across the lock.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{self, BoxStream};

use crate::error::{CrateError, Result};

use super::types::*;
use super::{AuthBackend, BlobStore, CloudBackend, DeviceRegistry, ManifestStore};
use std::sync::Arc;

#[derive(Default)]
struct MockState {
    manifest: Option<(Manifest, ManifestEtag)>,
    etag_counter: u64,
    blobs: HashMap<String, Bytes>,
    gc_queue: Vec<(GcEntryId, GcEntry)>,
    gc_counter: u64,
    devices: HashMap<String, DeviceRecord>,
}

#[derive(Clone, Default)]
pub struct MockCloudBackend {
    state: Arc<tokio::sync::Mutex<MockState>>,
}

impl MockCloudBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// A clone that shares the same backing store (a second device, one cloud).
    pub fn handle(&self) -> Self {
        self.clone()
    }
}

fn mock_session() -> AuthSession {
    AuthSession {
        uid: "mock-uid".into(),
        access_token: "mock-access".into(),
        refresh_token: "mock-refresh".into(),
        access_token_expires_at: SystemTime::now() + Duration::from_secs(3600),
        email: None,
        display_name: None,
    }
}

impl CloudBackend for MockCloudBackend {
    fn id(&self) -> &'static str {
        "mock"
    }
    fn auth(&self) -> Arc<dyn AuthBackend> {
        Arc::new(self.clone())
    }
    fn manifest(&self) -> Arc<dyn ManifestStore> {
        Arc::new(self.clone())
    }
    fn blobs(&self) -> Arc<dyn BlobStore> {
        Arc::new(self.clone())
    }
    fn devices(&self) -> Arc<dyn DeviceRegistry> {
        Arc::new(self.clone())
    }
}

#[async_trait]
impl AuthBackend for MockCloudBackend {
    async fn sign_in_with_idp(&self, _provider_id: &str, _id_token: &str) -> Result<AuthSession> {
        Ok(mock_session())
    }
    async fn refresh(&self, _refresh_token: &str) -> Result<AuthSession> {
        Ok(mock_session())
    }
    async fn sign_out(&self, _session: &AuthSession) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl ManifestStore for MockCloudBackend {
    async fn read(&self, _session: &AuthSession) -> Result<Option<(Manifest, ManifestEtag)>> {
        Ok(self.state.lock().await.manifest.clone())
    }

    async fn write(
        &self,
        _session: &AuthSession,
        manifest: &Manifest,
        expected: Option<&ManifestEtag>,
        gc_enqueue: &[GcEntry],
    ) -> Result<ManifestEtag> {
        let mut st = self.state.lock().await;
        let current = st.manifest.as_ref().map(|(_, e)| e.clone());
        if expected.cloned() != current {
            return Err(CrateError::CloudSyncConflict);
        }
        st.etag_counter += 1;
        let new_etag = ManifestEtag::counter(st.etag_counter);
        st.manifest = Some((manifest.clone(), new_etag.clone()));
        for entry in gc_enqueue {
            st.gc_counter += 1;
            let id = GcEntryId(format!("gc-{}", st.gc_counter));
            st.gc_queue.push((id, entry.clone()));
        }
        Ok(new_etag)
    }

    async fn subscribe(
        &self,
        _session: &AuthSession,
    ) -> Result<BoxStream<'static, (Manifest, ManifestEtag)>> {
        Ok(Box::pin(stream::empty()))
    }

    async fn dequeue_gc(
        &self,
        _session: &AuthSession,
        due_before: SystemTime,
        limit: usize,
    ) -> Result<Vec<(GcEntryId, GcEntry)>> {
        let st = self.state.lock().await;
        Ok(st
            .gc_queue
            .iter()
            .filter(|(_, e)| e.delete_after <= due_before)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn ack_gc(&self, _session: &AuthSession, id: GcEntryId) -> Result<()> {
        let mut st = self.state.lock().await;
        st.gc_queue.retain(|(qid, _)| *qid != id);
        Ok(())
    }
}

#[async_trait]
impl BlobStore for MockCloudBackend {
    async fn upload(
        &self,
        _session: &AuthSession,
        key: &str,
        data: Bytes,
        _content_type: &str,
    ) -> Result<()> {
        self.state.lock().await.blobs.insert(key.to_string(), data);
        Ok(())
    }

    async fn download(&self, _session: &AuthSession, key: &str) -> Result<Bytes> {
        self.state
            .lock()
            .await
            .blobs
            .get(key)
            .cloned()
            .ok_or_else(|| CrateError::CloudSyncBlobNotFound(key.to_string()))
    }

    async fn delete(&self, _session: &AuthSession, key: &str) -> Result<()> {
        self.state.lock().await.blobs.remove(key);
        Ok(())
    }
}

#[async_trait]
impl DeviceRegistry for MockCloudBackend {
    async fn upsert(&self, _session: &AuthSession, device: &DeviceRecord) -> Result<()> {
        self.state
            .lock()
            .await
            .devices
            .insert(device.device_id.clone(), device.clone());
        Ok(())
    }

    async fn list(&self, _session: &AuthSession) -> Result<Vec<DeviceRecord>> {
        Ok(self.state.lock().await.devices.values().cloned().collect())
    }

    async fn remove(&self, _session: &AuthSession, device_id: &str) -> Result<()> {
        self.state.lock().await.devices.remove(device_id);
        Ok(())
    }
}
