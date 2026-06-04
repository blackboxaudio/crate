//! Runtime state + orchestration for cloud sync, managed as Tauri state.
//!
//! Holds the backend, the live session, the sync status, and this device's identity,
//! and exposes the sign-in / push entry points shared by the commands
//! ([`crate::commands::cloud_sync`]) and the startup debounce loop ([`crate::run`]).
//! The DB `Mutex` guard is never held across an `.await` here; the session/status
//! locks are async (`tokio::sync::RwLock`) and held only briefly.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::error::{CrateError, Result};

use super::auth;
use super::backend::types::{AuthSession, DeviceRecord};
use super::backend::CloudBackend;
use super::config::CloudConfig;
use super::pipeline::merge::OverrideEvent;
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
    /// A transient connectivity failure: sync is paused and will retry automatically.
    /// Distinct from `Error` so a dropped network doesn't read as a hard failure.
    Offline,
    /// The last operation failed (see `last_error`).
    Error,
}

/// Which phase a failed sync op should surface: a transient connectivity error becomes
/// `Offline` (paused + auto-retry), anything else a hard `Error`.
fn phase_for_error(e: &CrateError) -> SyncPhase {
    if e.is_transient() {
        SyncPhase::Offline
    } else {
        SyncPhase::Error
    }
}

/// First-sign-in onboarding hint, surfaced ONLY on the sign-in response (never persisted
/// or polled), so it fires exactly once for the UI that initiated the sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnboardingKind {
    /// No vault yet — this is the first device; the UI kicks an initial push to create it.
    Initial,
    /// A vault exists — this is a fresh device; the UI pulls it and prompts the library-
    /// roots wizard so the restored tracks become playable.
    Restore,
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
    /// Onboarding hint — set ONLY on the [`CloudSyncState::sign_in`] response, always
    /// `None` in a polled status.
    pub onboarding: Option<OnboardingKind>,
}

/// A resolved override notification emitted to the UI on the `cloud-sync-override` event:
/// this device's value for `label` was replaced by the value from device `device`.
#[derive(Clone, Debug, Serialize)]
pub struct OverrideNotice {
    pub label: String,
    pub device: String,
}

pub struct CloudSyncState {
    backend: Option<Arc<dyn CloudBackend>>,
    config: Option<CloudConfig>,
    conn: Arc<Mutex<Connection>>,
    device_id: String,
    device_name: String,
    app_version: String,
    app_handle: AppHandle,
    session: RwLock<Option<AuthSession>>,
    status: RwLock<SyncStatus>,
    /// Serializes sync operations (the poll loop's pull/push plus a manual "Sync now")
    /// so they never overlap or fight over [`SyncStatus`]. Held across the whole op.
    sync_lock: tokio::sync::Mutex<()>,
    /// The last ~20 override notices, kept in memory for diagnostics (no audit-log UI in
    /// v1). Newest at the back.
    override_buffer: RwLock<VecDeque<OverrideNotice>>,
}

impl CloudSyncState {
    pub fn new(
        backend: Option<Arc<dyn CloudBackend>>,
        config: Option<CloudConfig>,
        conn: Arc<Mutex<Connection>>,
        device_id: String,
        device_name: String,
        app_version: String,
        app_handle: AppHandle,
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
            onboarding: None,
        };
        Self {
            backend,
            config,
            conn,
            device_id,
            device_name,
            app_version,
            app_handle,
            session: RwLock::new(None),
            status: RwLock::new(status),
            sync_lock: tokio::sync::Mutex::new(()),
            override_buffer: RwLock::new(VecDeque::new()),
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

    /// The last ~20 override notices (oldest first), kept in memory for diagnostics.
    pub async fn recent_overrides(&self) -> Vec<OverrideNotice> {
        self.override_buffer.read().await.iter().cloned().collect()
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
            Err(e) => {
                log::warn!("cloud_sync: session restore failed: {e}");
                if e.is_transient() {
                    if let Ok((email, display_name, photo_url)) =
                        auth::read_profile(&self.conn)
                    {
                        let mut st = self.status.write().await;
                        st.phase = SyncPhase::Offline;
                        st.email = email;
                        st.display_name = display_name;
                        st.photo_url = photo_url;
                        st.last_error = Some(e.to_string());
                    }
                }
            }
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
        let device_for_hb = self.device_record().await;
        tokio::spawn(async move {
            if let Err(e) = backend_for_hb
                .devices()
                .upsert(&session_for_hb, &device_for_hb)
                .await
            {
                log::warn!("cloud_sync: initial device heartbeat failed: {e}");
                return;
            }
            // Re-authorize this device: clear any prior revocation so signing in again
            // reconnects a device that was previously revoked from elsewhere.
            if let Err(e) = backend_for_hb
                .devices()
                .set_revoked(&session_for_hb, &device_for_hb.device_id, false)
                .await
            {
                log::warn!("cloud_sync: clearing revocation on sign-in failed: {e}");
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

        // Onboarding hint (return-only): does this account already have a vault?
        // `Restore` → fresh device, the UI pulls + prompts the roots wizard; `Initial` →
        // first device, the UI kicks an initial push to create the vault.
        let onboarding = match backend.manifest().read(&session).await {
            Ok(Some(_)) => Some(OnboardingKind::Restore),
            Ok(None) => Some(OnboardingKind::Initial),
            Err(e) => {
                log::warn!("cloud_sync: onboarding vault check failed: {e}");
                None
            }
        };

        *self.session.write().await = Some(session);

        let mut status = self.get_status().await;
        status.onboarding = onboarding;
        Ok(status)
    }

    /// Sign out: clear the stored refresh token + in-memory session.
    pub async fn sign_out(&self) -> Result<()> {
        let session = self.session.write().await.take();
        match &self.backend {
            Some(backend) => {
                auth::sign_out(backend, session.as_ref(), self.conn.clone()).await?
            }
            None => {
                let guard = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
                auth::token_store::clear_refresh_token(&guard)?;
            }
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
    /// session, sets status, and reports a device heartbeat on success. A transient
    /// connectivity failure surfaces as `Offline` (paused + auto-retry); a real failure
    /// as `Error`.
    pub async fn run_push(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let _sync = self.sync_lock.lock().await;
        let result = self.do_push(&backend).await;
        if let Err(e) = &result {
            self.set_phase(phase_for_error(e), Some(e.to_string())).await;
        }
        result
    }

    async fn do_push(&self, backend: &Arc<dyn CloudBackend>) -> Result<()> {
        let session = self
            .ensure_session(backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        // Bail before the heartbeat upsert if we've been revoked — otherwise the upsert
        // would resurrect this device's record.
        if self.enforce_not_revoked(backend, &session).await? {
            return Ok(());
        }

        self.set_phase(SyncPhase::Syncing, None).await;
        let overrides = push::push(self.conn.clone(), backend, &session, &self.device_id).await?;
        let record = self.device_record().await;
        let _ = backend.devices().upsert(&session, &record).await;
        self.emit_overrides(backend, &session, overrides).await;
        // Best-effort: refresh cached profile so a Google avatar/name change shows up
        // after a sync.
        if let Err(e) = self.refresh_profile(backend, &session).await {
            log::warn!("cloud_sync: profile refresh failed: {e}");
        }
        self.mark_synced().await;
        Ok(())
    }

    /// Pull other devices' changes now (the poll loop). Resolves a fresh session and
    /// merges any remote-ahead buckets. A successful merge marks the status synced; a
    /// no-op poll only clears a lingering `Offline` (connectivity returned) without
    /// flickering the indicator. Transient failures surface as `Offline`, real ones as
    /// `Error`. [`pull::pull`]'s etag gate keeps idle polls cheap.
    pub async fn run_pull(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let _sync = self.sync_lock.lock().await;
        let result = self.do_pull(&backend).await;
        match &result {
            Ok(()) => self.clear_offline_phase().await,
            Err(e) => self.set_phase(phase_for_error(e), Some(e.to_string())).await,
        }
        result
    }

    async fn do_pull(&self, backend: &Arc<dyn CloudBackend>) -> Result<()> {
        let session = self
            .ensure_session(backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;
        if self.enforce_not_revoked(backend, &session).await? {
            return Ok(());
        }
        let outcome = pull::pull(self.conn.clone(), backend, &session, &self.device_id).await?;
        self.emit_overrides(backend, &session, outcome.overrides).await;
        if outcome.merged {
            self.mark_synced().await;
        }
        Ok(())
    }

    /// Resolve override winners → device names, remember the last ~20, and emit the
    /// `cloud-sync-override` event so the loser device can toast. Best-effort and silent
    /// on failure — overrides are observational and never block a sync.
    async fn emit_overrides(
        &self,
        backend: &Arc<dyn CloudBackend>,
        session: &AuthSession,
        overrides: Vec<OverrideEvent>,
    ) {
        if overrides.is_empty() {
            return;
        }
        // One device-list read resolves every winner id → name (no DB guard held).
        let names: HashMap<String, String> = match backend.devices().list(session).await {
            Ok(devs) => devs.into_iter().map(|d| (d.device_id, d.name)).collect(),
            Err(e) => {
                log::warn!("cloud_sync: resolving override winner names failed: {e}");
                HashMap::new()
            }
        };
        let notices: Vec<OverrideNotice> = overrides
            .into_iter()
            .map(|ev| OverrideNotice {
                label: ev.label,
                device: names
                    .get(&ev.winner_device_id)
                    .cloned()
                    .unwrap_or_else(|| ev.winner_device_id.clone()),
            })
            .collect();
        {
            let mut buf = self.override_buffer.write().await;
            for n in &notices {
                buf.push_back(n.clone());
            }
            while buf.len() > 20 {
                buf.pop_front();
            }
        }
        if let Err(e) = self.app_handle.emit("cloud-sync-override", &notices) {
            log::warn!("cloud_sync: emit override event failed: {e}");
        }
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

    /// List devices registered against the signed-in account (revoked devices hidden —
    /// they've been cut off and will sign themselves out).
    pub async fn list_devices(&self) -> Result<Vec<DeviceRecord>> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;
        let devices = backend.devices().list(&session).await?;
        Ok(devices.into_iter().filter(|d| !d.revoked).collect())
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
    /// from the database. Caches the result. `None` means signed out.
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

    async fn device_record(&self) -> DeviceRecord {
        let st = self.status.read().await;
        DeviceRecord {
            device_id: st.device_id.clone(),
            name: st.device_name.clone(),
            last_seen: std::time::SystemTime::now(),
            app_version: self.app_version.clone(),
            revoked: false,
        }
    }

    /// If this device has been revoked remotely, sign out locally and return `true` so
    /// the caller aborts the current sync (before any heartbeat that would resurrect the
    /// record). Reads a single device doc — cheap enough to run on every push/pull.
    async fn enforce_not_revoked(
        &self,
        backend: &Arc<dyn CloudBackend>,
        session: &AuthSession,
    ) -> Result<bool> {
        let revoked = backend
            .devices()
            .get(session, &self.device_id)
            .await?
            .map(|d| d.revoked)
            .unwrap_or(false);
        if revoked {
            log::info!("cloud_sync: this device was revoked remotely; signing out");
            self.sign_out().await?;
        }
        Ok(revoked)
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

    /// Recover from `Offline` once connectivity returns: flip `Offline` → `Idle` after a
    /// successful round-trip that didn't itself merge anything. Leaves `Error` alone — a
    /// successful pull doesn't prove a failing push will now succeed.
    async fn clear_offline_phase(&self) {
        let mut st = self.status.write().await;
        if st.phase == SyncPhase::Offline {
            st.phase = SyncPhase::Idle;
            st.last_error = None;
        }
    }

    /// Re-fetch the user's current profile from the backend (Firebase
    /// `accounts:lookup`) and update both the live session, the visible status,
    /// and the cached profile rows. Picks up Google profile-picture/name changes
    /// without requiring a sign-out + sign-in.
    async fn refresh_profile(
        &self,
        backend: &Arc<dyn CloudBackend>,
        session: &AuthSession,
    ) -> Result<()> {
        let profile = backend.auth().lookup_profile(session).await?;
        if let Some(s) = self.session.write().await.as_mut() {
            s.email = profile.email.clone();
            s.display_name = profile.display_name.clone();
            s.photo_url = profile.photo_url.clone();
        }
        {
            let mut st = self.status.write().await;
            st.email = profile.email.clone();
            st.display_name = profile.display_name.clone();
            st.photo_url = profile.photo_url.clone();
        }
        auth::persist_profile_fields(
            &self.conn,
            profile.email.as_deref(),
            profile.display_name.as_deref(),
            profile.photo_url.as_deref(),
        )?;
        Ok(())
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

    /// Rename this device. The local name updates immediately; the Firebase
    /// heartbeat is best-effort — if it fails (e.g. offline), the next automatic
    /// heartbeat from `do_push` carries the new name.
    pub async fn rename_device(&self, name: &str) {
        {
            let mut st = self.status.write().await;
            st.device_name = name.to_string();
        }
        if let Ok(guard) = self.conn.lock() {
            let _ = auth::write_state(&guard, "device_name", name);
        }

        if let Some(backend) = self.backend.clone() {
            if let Ok(Some(session)) = self.ensure_session(&backend).await {
                let record = self.device_record().await;
                if let Err(e) = backend.devices().upsert(&session, &record).await {
                    log::warn!("cloud_sync: rename heartbeat failed (will retry): {e}");
                }
            }
        }
    }

    /// Revoke a device. If it's the current device, also signs out.
    pub async fn revoke_device(&self, device_id: &str) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        // Set a persistent revocation flag (disjoint from the heartbeat) so the target
        // device notices on its next push/pull and stops syncing. A bare `remove` would
        // be resurrected by that device's next heartbeat.
        backend
            .devices()
            .set_revoked(&session, device_id, true)
            .await?;

        if device_id == self.device_id {
            self.sign_out().await?;
        }
        Ok(())
    }

    /// Delete the user's entire cloud vault: every Storage blob (current + superseded),
    /// the Firestore manifest, the GC queue, and all device records. Local library data
    /// is left untouched. Signs out afterward — the account has no vault now; signing in
    /// again re-creates it from the local library.
    pub async fn delete_cloud_vault(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let session = self
            .ensure_session(&backend)
            .await?
            .ok_or_else(|| CrateError::CloudSyncAuth("not signed in".into()))?;

        let _sync = self.sync_lock.lock().await;
        let store = backend.manifest();
        let blobs = backend.blobs();

        // 1. Delete the current bucket blobs referenced by the manifest.
        if let Some((manifest, _)) = store.read(&session).await? {
            for entry in manifest.buckets.values() {
                let key = format!("users/{}/vault/{}", session.uid, entry.object_key);
                if let Err(e) = blobs.delete(&session, &key).await {
                    log::warn!("cloud_sync: vault delete blob failed: {e}");
                }
            }
        }

        // 2. Drain the GC queue (superseded blobs + their queue docs). `due_before` is far
        // in the future so every entry is returned regardless of its grace window.
        let far_future = std::time::SystemTime::now()
            + std::time::Duration::from_secs(100 * 365 * 24 * 3600);
        for _ in 0..100 {
            let due = store.dequeue_gc(&session, far_future, 300).await?;
            if due.is_empty() {
                break;
            }
            for (id, entry) in due {
                let _ = blobs.delete(&session, &entry.object_key).await;
                let _ = store.ack_gc(&session, id).await;
            }
        }

        // 3. Delete the manifest document.
        store.delete(&session).await?;

        // 4. Remove every device record (including this one).
        if let Ok(devices) = backend.devices().list(&session).await {
            for d in devices {
                let _ = backend.devices().remove(&session, &d.device_id).await;
            }
        }

        // 5. Reset local sync watermarks + dirty queue so a fresh sign-in re-pushes cleanly.
        {
            let guard = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
            guard.execute("DELETE FROM sync_dirty_buckets", [])?;
            guard.execute(
                "DELETE FROM sync_state WHERE key IN ('last_synced_manifest_etag', 'last_synced_manifest_hlc')",
                [],
            )?;
        }

        // 6. Sign out — the account has no vault now.
        self.sign_out().await?;
        Ok(())
    }
}
