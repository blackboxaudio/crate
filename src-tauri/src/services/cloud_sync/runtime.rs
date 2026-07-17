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
use super::synclog::SyncLog;

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

/// A coarse category for the last sync failure, so the UI can show a meaningful,
/// actionable message instead of a generic "Sync error" (the sanitized detail rides
/// along in `last_error`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncErrorKind {
    /// Transport-level connectivity failure (connect/timeout/DNS).
    Network,
    /// The session is invalid — the user needs to sign in again.
    Auth,
    /// The backend refused the request (security rules / App Check).
    Permission,
    /// Rate-limited or out of quota.
    Quota,
    /// The server rejected an upload as too large.
    TooLarge,
    /// Manifest CAS retries exhausted — another device kept winning the race.
    Conflict,
    /// The backend returned a server error.
    Server,
    /// A pulled bucket failed to apply to the local database.
    Merge,
    Unknown,
}

impl SyncErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            SyncErrorKind::Network => "network",
            SyncErrorKind::Auth => "auth",
            SyncErrorKind::Permission => "permission",
            SyncErrorKind::Quota => "quota",
            SyncErrorKind::TooLarge => "toolarge",
            SyncErrorKind::Conflict => "conflict",
            SyncErrorKind::Server => "server",
            SyncErrorKind::Merge => "merge",
            SyncErrorKind::Unknown => "unknown",
        }
    }
}

/// True when an op failed because the backend rejected the access token outright —
/// worth one silent session refresh + retry before surfacing an auth error.
fn is_unauthorized(r: &Result<()>) -> bool {
    matches!(r, Err(CrateError::CloudSyncHttp { status: 401, .. }))
}

fn classify_error(e: &CrateError) -> SyncErrorKind {
    match e {
        CrateError::CloudSyncNetwork(_) => SyncErrorKind::Network,
        CrateError::CloudSyncAuth(_) => SyncErrorKind::Auth,
        CrateError::CloudSyncHttp { status, code, .. } => match status {
            401 => SyncErrorKind::Auth,
            403 => SyncErrorKind::Permission,
            413 => SyncErrorKind::TooLarge,
            429 => SyncErrorKind::Quota,
            _ if *status >= 500 => SyncErrorKind::Server,
            _ if code.contains("QUOTA") => SyncErrorKind::Quota,
            _ => SyncErrorKind::Unknown,
        },
        CrateError::CloudSyncConflict => SyncErrorKind::Conflict,
        CrateError::CloudSyncMerge { .. } | CrateError::Database(_) => SyncErrorKind::Merge,
        _ => SyncErrorKind::Unknown,
    }
}

/// A hard auth error meaning the user's credential is gone / unusable — from `accounts:delete`
/// OR a session-refresh rejection — so account deletion is a no-op on the server and we just
/// clear local state. Deliberately EXCLUDES `CREDENTIAL_TOO_OLD_LOGIN_AGAIN` and permission /
/// App Check (403 `PERMISSION_DENIED`) failures: those must abort and keep local state rather
/// than falsely report the account deleted.
fn is_already_deleted_auth_error(e: &CrateError) -> bool {
    matches!(
        e,
        CrateError::CloudSyncAuth(msg)
            if msg.contains("USER_NOT_FOUND")        // accounts:delete / refresh: user is gone
                || msg.contains("INVALID_ID_TOKEN")      // accounts:delete: idToken no longer valid
                || msg.contains("EMAIL_NOT_FOUND")       // accounts:delete: user is gone
                || msg.contains("INVALID_REFRESH_TOKEN") // refresh: credential revoked
                || msg.contains("TOKEN_EXPIRED")         // refresh: credential expired
                || msg.contains("USER_DISABLED")         // refresh: account disabled (can't self-delete)
    )
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
    /// Category of `last_error`, when the last operation failed.
    pub last_error_kind: Option<SyncErrorKind>,
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

/// What the frontend needs to drive the native mobile auth session: the consent URL to open and
/// the callback scheme `ASWebAuthenticationSession` / Custom Tabs should intercept. Returned by
/// [`CloudSyncState::begin_sign_in`].
#[cfg(feature = "mobile")]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginSignIn {
    pub auth_url: String,
    pub callback_scheme: String,
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
    /// Persistent rotating log of sync outcomes (see [`SyncLog`]).
    sync_log: SyncLog,
    /// Serializes sync operations (the poll loop's pull/push plus a manual "Sync now")
    /// so they never overlap or fight over [`SyncStatus`]. Held across the whole op.
    sync_lock: tokio::sync::Mutex<()>,
    /// The last ~20 override notices, kept in memory for diagnostics (no audit-log UI in
    /// v1). Newest at the back.
    override_buffer: RwLock<VecDeque<OverrideNotice>>,
    /// PKCE verifier + CSRF state for an in-flight native mobile sign-in, stashed between the
    /// `begin_sign_in` and `complete_sign_in` IPC calls. Single-slot: a new `begin` supersedes
    /// any abandoned attempt. The verifier never leaves Rust.
    #[cfg(feature = "mobile")]
    pending_auth: tokio::sync::Mutex<Option<auth::oauth_flow::PendingAuth>>,
}

impl CloudSyncState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        backend: Option<Arc<dyn CloudBackend>>,
        config: Option<CloudConfig>,
        conn: Arc<Mutex<Connection>>,
        device_id: String,
        device_name: String,
        app_version: String,
        app_handle: AppHandle,
        app_data_dir: &std::path::Path,
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
            last_error_kind: None,
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
            sync_log: SyncLog::new(app_data_dir),
            sync_lock: tokio::sync::Mutex::new(()),
            override_buffer: RwLock::new(VecDeque::new()),
            #[cfg(feature = "mobile")]
            pending_auth: tokio::sync::Mutex::new(None),
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

    /// Human-readable diagnostics for the "Copy sync diagnostics" affordance: a status
    /// header plus the tail of the persistent sync log. Everything in here is already
    /// sanitized at the error-construction layer (no URLs / API keys).
    pub async fn diagnostics(&self) -> String {
        let st = self.status.read().await.clone();
        format!(
            "Crate sync diagnostics\nversion: {}\nplatform: {}\ndevice: {} ({})\nphase: {:?}\nlast_error: {}\nlast_error_kind: {}\nlast_synced_at: {}\n--- sync.log (tail) ---\n{}",
            self.app_version,
            std::env::consts::OS,
            st.device_name,
            st.device_id,
            st.phase,
            st.last_error.as_deref().unwrap_or("-"),
            st.last_error_kind.map(|k| k.as_str()).unwrap_or("-"),
            st.last_synced_at.as_deref().unwrap_or("-"),
            self.sync_log.tail(64 * 1024),
        )
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
                    if let Ok((email, display_name, photo_url)) = auth::read_profile(&self.conn) {
                        let mut st = self.status.write().await;
                        st.phase = SyncPhase::Offline;
                        st.email = email;
                        st.display_name = display_name;
                        st.photo_url = photo_url;
                        st.last_error = Some(e.to_string());
                        st.last_error_kind = Some(classify_error(&e));
                    }
                }
            }
        }
    }

    /// Run the full desktop sign-in flow for `provider_id` (e.g. `"google"`). `open_url` opens
    /// the consent screen in the system browser.
    #[cfg(feature = "desktop")]
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

        self.finish_session_setup(backend, session).await
    }

    /// Begin a native mobile sign-in: build the consent URL + PKCE secrets, stash the secrets
    /// server-side, and return the URL + callback scheme for the frontend's native auth session
    /// (`ASWebAuthenticationSession` on iOS / Custom Tabs on Android, via `tauri-plugin-web-auth`).
    #[cfg(feature = "mobile")]
    pub async fn begin_sign_in(&self, provider_id: &str) -> Result<BeginSignIn> {
        // Fail fast if sync isn't configured (no backend → sign-in is impossible).
        let _backend = self.require_backend()?;
        let config = self
            .config
            .clone()
            .ok_or_else(|| CrateError::CloudSync("cloud sync not configured".into()))?;
        let provider = auth::providers::provider_by_id(provider_id)
            .ok_or_else(|| CrateError::CloudSyncAuth(format!("unknown provider {provider_id}")))?;

        let (client_id, callback_scheme) = config.mobile_client()?;
        let redirect_uri = format!("{callback_scheme}:/oauth2redirect");
        let (auth_url, pending) =
            auth::oauth_flow::build_auth_request(provider.as_ref(), &client_id, &redirect_uri);
        *self.pending_auth.lock().await = Some(pending);
        Ok(BeginSignIn {
            auth_url,
            callback_scheme,
        })
    }

    /// Complete a native mobile sign-in with the `code`/`state` the frontend extracted from the
    /// OAuth callback URL. Validates `state` against the stashed PKCE secrets, exchanges the code
    /// for the provider ID token, then runs the shared session-setup tail.
    #[cfg(feature = "mobile")]
    pub async fn complete_sign_in(&self, code: &str, state: &str) -> Result<SyncStatus> {
        let backend = self.require_backend()?;
        let config = self
            .config
            .clone()
            .ok_or_else(|| CrateError::CloudSync("cloud sync not configured".into()))?;

        let pending = self
            .pending_auth
            .lock()
            .await
            .take()
            .ok_or_else(|| CrateError::CloudSyncAuth("no sign-in in progress".into()))?;
        if pending.state != state {
            return Err(CrateError::CloudSyncAuth("state mismatch".into()));
        }
        let provider = auth::providers::provider_by_id(&pending.provider_id).ok_or_else(|| {
            CrateError::CloudSyncAuth(format!("unknown provider {}", pending.provider_id))
        })?;

        // Same client id + redirect_uri the consent request used (both derived from config).
        let (client_id, callback_scheme) = config.mobile_client()?;
        let redirect_uri = format!("{callback_scheme}:/oauth2redirect");
        let session = auth::complete_sign_in_with_code(
            &backend,
            provider.as_ref(),
            self.conn.clone(),
            &client_id,
            &redirect_uri,
            code,
            &pending.verifier,
        )
        .await?;

        self.finish_session_setup(backend, session).await
    }

    /// Native iOS **Sign in with Apple**: present the AuthenticationServices sheet, exchange the
    /// Apple identity token into Firebase (`providerId = apple.com`, with the raw nonce), then run
    /// the shared session-setup tail. Apple returns the user's name only on the FIRST authorization
    /// and its identity token carries none, so capture it — falling back to the name cached on the
    /// first sign-in — and persist it as the display name, otherwise
    /// [`finish_sign_in`](auth::finish_sign_in)'s profile write would blank it on later sign-ins.
    #[cfg(target_os = "ios")]
    pub async fn sign_in_with_apple(&self) -> Result<SyncStatus> {
        let backend = self.require_backend()?;
        // Fail fast if sync isn't configured (mirrors begin_sign_in).
        let _config = self
            .config
            .clone()
            .ok_or_else(|| CrateError::CloudSync("cloud sync not configured".into()))?;

        let cred = auth::apple_native::request_apple_credential(&self.app_handle).await?;

        // Prefer the freshly-captured name; on repeat sign-ins (where Apple omits it) fall back to
        // the name we cached the first time.
        let cached_name = auth::read_profile(&self.conn)?.1;
        let name = cred.full_name.clone().or(cached_name);

        let mut session = auth::finish_sign_in(
            &backend,
            "apple.com",
            Some(&cred.raw_nonce),
            self.conn.clone(),
            &cred.identity_token,
        )
        .await?;

        // Reflect + persist the resolved name: the Apple token has no name claim, so
        // finish_sign_in's persist_profile just wrote it blank.
        session.display_name = session.display_name.or_else(|| name.clone());
        auth::persist_profile_fields(
            &self.conn,
            session.email.as_deref(),
            name.as_deref(),
            session.photo_url.as_deref(),
        )?;

        self.finish_session_setup(backend, session).await
    }

    /// Shared post-token tail for desktop and mobile sign-in: fire the best-effort device
    /// heartbeat, update status, compute the one-shot onboarding hint, store the session, and
    /// return the status snapshot (with `onboarding` set).
    async fn finish_session_setup(
        &self,
        backend: Arc<dyn CloudBackend>,
        session: AuthSession,
    ) -> Result<SyncStatus> {
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
            st.last_error_kind = None;
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
            Some(backend) => auth::sign_out(backend, session.as_ref(), self.conn.clone()).await?,
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
        st.last_error_kind = None;
        Ok(())
    }

    /// Run a push now (manual "Sync now" or the debounce loop). Resolves a fresh
    /// session, sets status, and reports a device heartbeat on success. A transient
    /// connectivity failure surfaces as `Offline` (paused + auto-retry); a real failure
    /// as `Error`.
    pub async fn run_push(&self) -> Result<()> {
        let backend = self.require_backend()?;
        let _sync = self.sync_lock.lock().await;
        let started = std::time::Instant::now();
        let mut result = self.do_push(&backend).await;
        if is_unauthorized(&result) {
            // The token was rejected even though `ensure_fresh` considered it valid
            // (clock skew / server-side revocation). Drop the cached session so the
            // retry re-mints from the stored refresh token; a second 401 is a real
            // auth failure and surfaces as such.
            self.sync_log
                .append("push got HTTP 401 — refreshing session, retrying once");
            *self.session.write().await = None;
            result = self.do_push(&backend).await;
        }
        match &result {
            Ok(()) => self.sync_log.append(&format!(
                "push ok in {:.1}s",
                started.elapsed().as_secs_f32()
            )),
            Err(e) => self.set_error_status("push", e).await,
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
        let outcome = push::push(self.conn.clone(), backend, &session, &self.device_id).await?;
        if !outcome.uploaded.is_empty() {
            let summary: Vec<String> = outcome
                .uploaded
                .iter()
                .map(|(name, bytes)| format!("{name} ({:.1} KiB raw)", *bytes as f64 / 1024.0))
                .collect();
            self.sync_log
                .append(&format!("push uploaded: {}", summary.join(", ")));
        }
        let record = self.device_record().await;
        let _ = backend.devices().upsert(&session, &record).await;
        self.emit_overrides(backend, &session, outcome.overrides)
            .await;
        // A push pull-then-merges the remote union before uploading; reload the affected
        // UI stores so a peer's change shows after a manual "Sync now" too.
        self.emit_merged(outcome.merged_buckets);
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
        let mut result = self.do_pull(&backend).await;
        if is_unauthorized(&result) {
            self.sync_log
                .append("pull got HTTP 401 — refreshing session, retrying once");
            *self.session.write().await = None;
            result = self.do_pull(&backend).await;
        }
        match &result {
            Ok(()) => self.clear_offline_phase().await,
            Err(e) => self.set_error_status("pull", e).await,
        }
        result
    }

    /// One opportunistic sync pass: pull other devices' changes, then push local edits when the
    /// dirty queue is non-empty. A no-op when signed out. Shared by the `sync_foreground`
    /// command (app launch / foreground return) and the iOS BGTaskScheduler background handler,
    /// which reuses this managed state so the pass still enforces revocation and heartbeats.
    pub async fn run_foreground_pass(&self) -> Result<()> {
        if !self.is_signed_in().await {
            return Ok(());
        }
        self.run_pull().await?;
        if self.dirty_quiescent(std::time::Duration::ZERO)? {
            self.run_push().await?;
        }
        Ok(())
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
        let merged = outcome.merged;
        if merged {
            self.sync_log
                .append(&format!("pull merged: {}", outcome.buckets.join(", ")));
        }
        if !outcome.skipped.is_empty() {
            self.sync_log.append(&format!(
                "pull skipped missing blob(s): {} (re-upload scheduled)",
                outcome.skipped.join(", ")
            ));
        }
        self.emit_overrides(backend, &session, outcome.overrides)
            .await;
        // Tell the UI which stores to reload so a peer's change shows without a restart.
        self.emit_merged(outcome.buckets);
        if merged {
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

    /// Emit `cloud-sync-merged` with the deduped plain names of the buckets a pull/push
    /// just merged from a peer (e.g. `["playlists", "tracks/3"]`), so the frontend reloads
    /// only the affected stores. No-op when nothing merged. Best-effort and silent on
    /// failure — a missed reload self-corrects on the next merge or app restart.
    fn emit_merged(&self, mut buckets: Vec<String>) {
        if buckets.is_empty() {
            return;
        }
        buckets.sort();
        buckets.dedup();
        if let Err(e) = self.app_handle.emit("cloud-sync-merged", &buckets) {
            log::warn!("cloud_sync: emit merged event failed: {e}");
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
        let mut devices: Vec<_> = backend
            .devices()
            .list(&session)
            .await?
            .into_iter()
            .filter(|d| !d.revoked)
            .collect();

        // Local device name is authoritative — the Firebase heartbeat may lag.
        let local_name = self.status.read().await.device_name.clone();
        if let Some(me) = devices.iter_mut().find(|d| d.device_id == self.device_id) {
            me.name = local_name;
        }
        Ok(devices)
    }

    /// True when the dirty queue is non-empty and has been quiet for `quiescent`
    /// (the debounce condition). Reads `sync_dirty_buckets.marked_at` (RFC 3339).
    pub fn dirty_quiescent(&self, quiescent: std::time::Duration) -> Result<bool> {
        let guard = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let newest: Option<String> =
            guard.query_row("SELECT MAX(marked_at) FROM sync_dirty_buckets", [], |r| {
                r.get::<_, Option<String>>(0)
            })?;
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
    async fn ensure_session(&self, backend: &Arc<dyn CloudBackend>) -> Result<Option<AuthSession>> {
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

    /// Record a failed sync op: phase (Offline vs Error), the sanitized error text, its
    /// category for the UI, and a sync-log line for later diagnosis.
    async fn set_error_status(&self, op: &str, e: &CrateError) {
        let phase = phase_for_error(e);
        let kind = classify_error(e);
        self.sync_log.append(&format!(
            "{op} failed kind={} phase={:?}: {e}",
            kind.as_str(),
            phase
        ));
        if e.is_transient() {
            log::debug!("cloud_sync: {op} failed (transient): {e}");
        } else {
            log::warn!("cloud_sync: {op} failed: {e}");
        }
        let mut st = self.status.write().await;
        st.phase = phase;
        st.last_error = Some(e.to_string());
        st.last_error_kind = Some(kind);
    }

    async fn mark_synced(&self) {
        let mut st = self.status.write().await;
        st.phase = SyncPhase::Idle;
        st.last_error = None;
        st.last_error_kind = None;
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
            st.last_error_kind = None;
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
    /// heartbeat is fire-and-forget — if it fails (e.g. offline), the next
    /// automatic heartbeat from `do_push` carries the new name.
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
                tokio::spawn(async move {
                    if let Err(e) = backend.devices().upsert(&session, &record).await {
                        log::warn!("cloud_sync: rename heartbeat failed (will retry): {e}");
                    }
                });
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

    /// Remote vault teardown + local watermark reset, shared by [`Self::delete_cloud_vault`]
    /// and [`Self::delete_account`]. The caller MUST already hold `self.sync_lock`. Does NOT
    /// sign out and does NOT clear the persisted profile keys — each caller owns its tail.
    ///
    /// A best-effort complete sweep runs first (every object under `users/{uid}/vault/`,
    /// covering blobs the manifest / GC queue may no longer reference), then the manifest +
    /// GC-queue enumeration as a fallback. Every delete is 404-idempotent, so re-running the
    /// whole teardown after a later-step failure is harmless.
    async fn teardown_vault_locked(
        &self,
        session: &AuthSession,
        backend: &Arc<dyn CloudBackend>,
    ) -> Result<()> {
        let store = backend.manifest();
        let blobs = backend.blobs();

        // 0. Complete sweep: list & delete every object under the vault prefix. Best-effort —
        // if listing is denied (Storage rules) or fails, fall through to the manifest / GC
        // enumeration below so teardown can never be *blocked* by a missing list permission.
        let prefix = format!("users/{}/vault/", session.uid);
        match blobs.list_prefix(session, &prefix).await {
            Ok(keys) => {
                for key in keys {
                    if let Err(e) = blobs.delete(session, &key).await {
                        log::warn!("cloud_sync: vault sweep delete failed: {e}");
                    }
                }
            }
            Err(e) => log::warn!(
                "cloud_sync: vault blob list failed ({e}); falling back to manifest/GC enumeration"
            ),
        }

        // 1. Delete the current bucket blobs referenced by the manifest.
        if let Some((manifest, _)) = store.read(session).await? {
            for entry in manifest.buckets.values() {
                let key = format!("users/{}/vault/{}", session.uid, entry.object_key);
                if let Err(e) = blobs.delete(session, &key).await {
                    log::warn!("cloud_sync: vault delete blob failed: {e}");
                }
            }
        }

        // 2. Drain the GC queue (superseded blobs + their queue docs). `due_before` is far
        // in the future so every entry is returned regardless of its grace window.
        let far_future =
            std::time::SystemTime::now() + std::time::Duration::from_secs(100 * 365 * 24 * 3600);
        for _ in 0..100 {
            let due = store.dequeue_gc(session, far_future, 300).await?;
            if due.is_empty() {
                break;
            }
            for (id, entry) in due {
                let _ = blobs.delete(session, &entry.object_key).await;
                let _ = store.ack_gc(session, id).await;
            }
        }

        // 3. Delete the manifest document.
        store.delete(session).await?;

        // 4. Remove every device record (including this one).
        if let Ok(devices) = backend.devices().list(session).await {
            for d in devices {
                let _ = backend.devices().remove(session, &d.device_id).await;
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
        self.teardown_vault_locked(&session, &backend).await?;
        // The account still exists — keep the cached profile keys for a clean re-sign-in.
        self.sign_out().await?;
        Ok(())
    }

    /// Permanently delete the user's account: all cloud data (vault teardown) followed by
    /// the Firebase Auth user itself (Identity Toolkit `accounts:delete`), then every local
    /// auth/profile trace. The in-app "Delete account" flow required by App Store Guideline
    /// 5.1.1(v) / Google Play. Local library data is left untouched.
    ///
    /// Idempotent + tolerant of already-deleted resources: if the session can't be refreshed
    /// (credential already revoked) or `accounts:delete` reports the user is already gone, the
    /// local state is still cleared and the call succeeds. Genuine / transient failures abort
    /// and keep local state so the user can retry.
    pub async fn delete_account(&self) -> Result<()> {
        let backend = self.require_backend()?;

        // Resolve the session, tolerating an already-invalid credential (idempotency 1 of 2).
        // For a Google-sign-in-only app a refresh rejection (INVALID_REFRESH_TOKEN /
        // USER_NOT_FOUND / USER_DISABLED / TOKEN_EXPIRED) means the account is already gone or
        // unusable — clear local state and report success. Transient errors abort / retry.
        let session = match self.ensure_session(&backend).await {
            Ok(Some(s)) => s,
            Ok(None) => return self.finalize_account_deletion().await,
            Err(ref e) if is_already_deleted_auth_error(e) => {
                log::warn!(
                    "cloud_sync: delete_account: session unusable ({e}); treating as already-deleted"
                );
                return self.finalize_account_deletion().await;
            }
            Err(e) => return Err(e),
        };

        let _sync = self.sync_lock.lock().await;

        // 1. Tear down all cloud data while the session is still valid.
        self.teardown_vault_locked(&session, &backend).await?;

        // 2. Delete the Firebase Auth user — MUST run before any local sign-out (it needs the
        // live idToken). An "already gone" auth error (idempotency 2 of 2) is success; a stale-
        // credential (CREDENTIAL_TOO_OLD_LOGIN_AGAIN) or transient error aborts so the user can
        // retry after re-authenticating, with local state preserved.
        match backend.auth().delete_account(&session).await {
            Ok(()) => {}
            Err(e) if is_already_deleted_auth_error(&e) => {
                log::warn!(
                    "cloud_sync: delete_account: auth user already gone ({e}); treating as success"
                );
            }
            Err(e) => return Err(e),
        }

        // 3. Clear every local auth/profile trace and reset to signed-out.
        self.finalize_account_deletion().await
    }

    /// Clear ALL persisted auth/profile state + reset the in-memory session/status to
    /// signed-out. Unlike [`Self::sign_out`] alone, this also drops the cached profile keys a
    /// permanently-deleted account can never reuse (`delete_cloud_vault` deliberately keeps
    /// them for a re-sign-in).
    async fn finalize_account_deletion(&self) -> Result<()> {
        {
            let guard = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
            guard.execute(
                "DELETE FROM sync_state WHERE key IN ('cloud_uid', 'cloud_email', 'cloud_display_name', 'cloud_photo_url')",
                [],
            )?;
        } // drop the guard before sign_out (auth::sign_out re-locks conn)
        self.sign_out().await
    }
}
