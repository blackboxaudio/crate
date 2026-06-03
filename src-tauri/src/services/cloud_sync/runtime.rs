//! Runtime state + orchestration for cloud sync, managed as Tauri state.
//!
//! Holds the backend, the live session, the sync status, and this device's identity,
//! and exposes the sign-in / push entry points shared by the commands
//! ([`crate::commands::cloud_sync`]) and the startup debounce loop ([`crate::run`]).
//! The DB `Mutex` guard is never held across an `.await` here; the session/status
//! locks are async (`tokio::sync::RwLock`) and held only briefly.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::Serialize;
use tokio::sync::RwLock;

use crate::error::{CrateError, Result};

use super::auth;
use super::backend::types::{AuthSession, DeviceRecord};
use super::backend::{CloudBackend, DeviceRegistry};
use super::config::CloudConfig;
use super::pipeline::push;

/// Coarse sync state surfaced to the UI (Phase 4) + status indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncPhase {
    /// No config / backend on this build — sync unavailable.
    Disabled,
    /// Configured but not signed in.
    SignedOut,
    /// Signed in, nothing in flight.
    Idle,
    /// A push is currently running.
    Syncing,
    /// The last operation failed (see `last_error`).
    Error,
}

/// A snapshot of sync status for the frontend.
#[derive(Clone, Debug, Serialize)]
pub struct SyncStatus {
    pub phase: SyncPhase,
    pub email: Option<String>,
    pub device_id: String,
    pub device_name: String,
    pub last_error: Option<String>,
    /// RFC 3339 timestamp of the last successful push, if any.
    pub last_synced_at: Option<String>,
}

pub struct CloudSyncState {
    backend: Option<Arc<dyn CloudBackend>>,
    config: Option<CloudConfig>,
    conn: Arc<Mutex<Connection>>,
    device_id: String,
    device_name: String,
    app_version: String,
    session: RwLock<Option<AuthSession>>,
    status: RwLock<SyncStatus>,
}

impl CloudSyncState {
    pub fn new(
        backend: Option<Arc<dyn CloudBackend>>,
        config: Option<CloudConfig>,
        conn: Arc<Mutex<Connection>>,
        device_id: String,
        device_name: String,
        app_version: String,
    ) -> Self {
        let phase = if backend.is_some() {
            SyncPhase::SignedOut
        } else {
            SyncPhase::Disabled
        };
        let status = SyncStatus {
            phase,
            email: None,
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            last_error: None,
            last_synced_at: None,
        };
        Self {
            backend,
            config,
            conn,
            device_id,
            device_name,
            app_version,
            session: RwLock::new(None),
            status: RwLock::new(status),
        }
    }

    /// Whether cloud sync is configured at all (a backend + config are present).
    pub fn is_available(&self) -> bool {
        self.backend.is_some() && self.config.is_some()
    }

    /// Whether a live session is loaded in memory.
    pub async fn is_signed_in(&self) -> bool {
        self.session.read().await.is_some()
    }

    /// A snapshot of the current status.
    pub async fn get_status(&self) -> SyncStatus {
        self.status.read().await.clone()
    }

    /// Restore a persisted session at startup (no-op if signed out / unconfigured).
    pub async fn restore_session(&self) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        match auth::current_session(&backend, self.conn.clone()).await {
            Ok(Some(session)) => {
                {
                    let mut st = self.status.write().await;
                    st.phase = SyncPhase::Idle;
                    st.email = session.email.clone();
                }
                *self.session.write().await = Some(session);
            }
            Ok(None) => {}
            Err(e) => log::warn!("cloud_sync: session restore failed: {e}"),
        }
    }

    /// Run the full sign-in flow for `provider_id` (e.g. `"google"`). `open_url` opens
    /// the consent screen in the system browser.
    pub async fn sign_in(
        &self,
        provider_id: &str,
        open_url: impl FnOnce(&str) -> Result<()> + Send,
    ) -> Result<SyncStatus> {
        let backend = self.require_backend()?;
        let config = self
            .config
            .clone()
            .ok_or_else(|| CrateError::CloudSync("cloud sync not configured".into()))?;
        let provider = auth::providers::provider_by_id(provider_id)
            .ok_or_else(|| CrateError::CloudSyncAuth(format!("unknown provider {provider_id}")))?;

        let session = auth::sign_in(
            &backend,
            &config,
            provider.as_ref(),
            self.conn.clone(),
            open_url,
        )
        .await?;

        // Best-effort device heartbeat.
        let _ = backend.devices().upsert(&session, &self.device_record()).await;

        {
            let mut st = self.status.write().await;
            st.phase = SyncPhase::Idle;
            st.email = session.email.clone();
            st.last_error = None;
        }
        *self.session.write().await = Some(session);
        Ok(self.get_status().await)
    }

    /// Sign out: clear the keychain refresh token + in-memory session.
    pub async fn sign_out(&self) -> Result<()> {
        let session = self.session.write().await.take();
        match &self.backend {
            Some(backend) => auth::sign_out(backend, session.as_ref()).await?,
            None => auth::keychain::clear_refresh_token()?,
        }
        let mut st = self.status.write().await;
        st.phase = if self.backend.is_some() {
            SyncPhase::SignedOut
        } else {
            SyncPhase::Disabled
        };
        st.email = None;
        st.last_error = None;
        Ok(())
    }

    /// Run a push now (manual "Sync now" or the debounce loop). Resolves a fresh
    /// session, sets status, and reports a device heartbeat on success.
    pub async fn run_push(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        self.set_phase(SyncPhase::Syncing, None).await;
        let result = push::push(self.conn.clone(), &backend, &session, &self.device_id).await;
        match &result {
            Ok(()) => {
                let _ = backend.devices().upsert(&session, &self.device_record()).await;
                self.mark_synced().await;
            }
            Err(e) => self.set_phase(SyncPhase::Error, Some(e.to_string())).await,
        }
        result
    }

    /// List devices registered against the signed-in account.
    pub async fn list_devices(&self) -> Result<Vec<DeviceRecord>> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;
        backend.devices().list(&session).await
    }

    /// True when the dirty queue is non-empty and has been quiet for `quiescent`
    /// (the debounce condition). Reads `sync_dirty_buckets.marked_at` (RFC 3339).
    pub fn dirty_quiescent(&self, quiescent: std::time::Duration) -> Result<bool> {
        let guard = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let newest: Option<String> = guard.query_row(
            "SELECT MAX(marked_at) FROM sync_dirty_buckets",
            [],
            |r| r.get::<_, Option<String>>(0),
        )?;
        let Some(newest) = newest else {
            return Ok(false); // empty queue
        };
        let Ok(marked) = chrono::DateTime::parse_from_rfc3339(&newest) else {
            return Ok(true); // unparseable timestamp → don't get stuck, push it
        };
        let age = chrono::Utc::now().signed_duration_since(marked.with_timezone(&chrono::Utc));
        Ok(age.to_std().map(|a| a >= quiescent).unwrap_or(false))
    }

    // --- internals --------------------------------------------------------------

    fn require_backend(&self) -> Result<Arc<dyn CloudBackend>> {
        self.backend
            .clone()
            .ok_or_else(|| CrateError::CloudSync("cloud sync not configured".into()))
    }

    /// Resolve a usable session: refresh the cached one if it's near expiry, else load
    /// from the keychain. Caches the result. `None` means signed out.
    async fn ensure_session(
        &self,
        backend: &Arc<dyn CloudBackend>,
    ) -> Result<Option<AuthSession>> {
        let cached = self.session.read().await.clone();
        let session = match cached {
            Some(s) => auth::ensure_fresh(backend, self.conn.clone(), s).await?,
            None => match auth::current_session(backend, self.conn.clone()).await? {
                Some(s) => s,
                None => return Ok(None),
            },
        };
        *self.session.write().await = Some(session.clone());
        Ok(Some(session))
    }

    fn device_record(&self) -> DeviceRecord {
        DeviceRecord {
            device_id: self.device_id.clone(),
            name: self.device_name.clone(),
            last_seen: std::time::SystemTime::now(),
            app_version: self.app_version.clone(),
        }
    }

    async fn set_phase(&self, phase: SyncPhase, error: Option<String>) {
        let mut st = self.status.write().await;
        st.phase = phase;
        if error.is_some() {
            st.last_error = error;
        }
    }

    async fn mark_synced(&self) {
        let mut st = self.status.write().await;
        st.phase = SyncPhase::Idle;
        st.last_error = None;
        st.last_synced_at = Some(chrono::Utc::now().to_rfc3339());
    }
}
