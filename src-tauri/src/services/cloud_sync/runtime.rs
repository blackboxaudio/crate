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
use super::backend::CloudBackend;
use super::config::CloudConfig;
use super::pipeline::{gc, pull, push};

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
    pub display_name: Option<String>,
    pub photo_url: Option<String>,
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
    /// Serializes sync operations (the poll loop's pull/push plus a manual "Sync now")
    /// so they never overlap or fight over [`SyncStatus`]. Held across the whole op.
    sync_lock: tokio::sync::Mutex<()>,
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
            display_name: None,
            photo_url: None,
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
            sync_lock: tokio::sync::Mutex::new(()),
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
                    st.display_name = session.display_name.clone();
                    st.photo_url = session.photo_url.clone();
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

        // Best-effort device heartbeat — don't block the sign-in return on it.
        let backend_for_hb = backend.clone();
        let session_for_hb = session.clone();
        let device_for_hb = self.device_record();
        tokio::spawn(async move {
            if let Err(e) = backend_for_hb
                .devices()
                .upsert(&session_for_hb, &device_for_hb)
                .await
            {
                log::warn!("cloud_sync: initial device heartbeat failed: {e}");
            }
        });

        {
            let mut st = self.status.write().await;
            st.phase = SyncPhase::Idle;
            st.email = session.email.clone();
            st.display_name = session.display_name.clone();
            st.photo_url = session.photo_url.clone();
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
        st.display_name = None;
        st.photo_url = None;
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

        let _sync = self.sync_lock.lock().await;
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

    /// Pull other devices' changes now (the poll loop). Resolves a fresh session and
    /// merges any remote-ahead buckets. Status is updated only when something actually
    /// merged — an unchanged poll is a silent no-op, so the indicator doesn't flicker
    /// on every tick. Returns via [`pull::pull`], whose etag gate keeps idle polls cheap.
    pub async fn run_pull(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        let _sync = self.sync_lock.lock().await;
        let result = pull::pull(self.conn.clone(), &backend, &session, &self.device_id).await;
        match &result {
            Ok(true) => self.mark_synced().await,
            Ok(false) => {} // nothing changed — leave status untouched
            Err(e) => self.set_phase(SyncPhase::Error, Some(e.to_string())).await,
        }
        result.map(|_| ())
    }

    /// Reclaim superseded blobs whose GC grace window has elapsed. Best-effort and run
    /// once per session at startup; does not touch [`SyncStatus`] (it's background
    /// cleanup, not a user-facing sync) and does not take the sync lock.
    pub async fn run_gc_sweep(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;
        let n = gc::gc_sweep(&backend, &session).await?;
        if n > 0 {
            log::info!("cloud_sync: gc sweep reclaimed {n} superseded blob(s)");
        }
        Ok(())
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

    // --- Phase 4: library roots + device management ---

    /// Run a synchronous closure with the database connection.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Connection) -> Result<T>,
    {
        let guard = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        f(&guard)
    }

    /// Rename this device and push a heartbeat.
    pub async fn rename_device(&self, name: &str) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        let mut record = self.device_record();
        record.name = name.to_string();
        backend.devices().upsert(&session, &record).await?;

        {
            let mut st = self.status.write().await;
            st.device_name = name.to_string();
        }
        Ok(())
    }

    /// Revoke a device. If it's the current device, also signs out.
    pub async fn revoke_device(&self, device_id: &str) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        backend.devices().remove(&session, device_id).await?;

        if device_id == self.device_id {
            self.sign_out().await?;
        }
        Ok(())
    }
}
