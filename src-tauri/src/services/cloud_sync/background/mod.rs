//! Opportunistic background cloud sync (mobile only).
//!
//! Two entry points share one goal — pull peers' changes, then push local edits — but differ
//! by how they obtain the sync dependencies:
//!
//! - **iOS** ([`ios`]): a `BGAppRefreshTask` handler that reuses the app's managed
//!   [`CloudSyncState`](super::runtime::CloudSyncState) (the app boots fully even on a
//!   background launch), calling `run_foreground_pass()` — the same path the app-foreground
//!   sync uses, so it also enforces revocation and heartbeats.
//! - **Android** ([`android`]): a headless WorkManager `Worker` that may cold-start the
//!   process without the Tauri Activity, so it rebuilds `{conn, backend, session}` from
//!   scratch and calls the bare [`run_background_sync`] core below.
//!
//! Neither path emits frontend status events — nothing is listening while backgrounded.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};

use super::backend::types::AuthSession;
use super::backend::CloudBackend;
use super::pipeline::{pull, push};

#[cfg(target_os = "ios")]
pub mod ios;

#[cfg(target_os = "android")]
pub mod android;

/// Schedule opportunistic background sync via the platform scheduler (called after sign-in).
/// Idempotent; a no-op on unsupported targets (e.g. a desktop-in-mobile-feature build).
pub fn schedule() {
    #[cfg(target_os = "ios")]
    ios::submit_refresh_request();
    #[cfg(target_os = "android")]
    android::ensure_scheduled();
}

/// Cancel scheduled background sync (called on sign-out). On iOS there is nothing to cancel —
/// the BGTask handler already no-ops when signed out — so this only unwinds Android's
/// WorkManager periodic work. No-op on unsupported targets.
pub fn cancel() {
    #[cfg(target_os = "android")]
    android::cancel_scheduled();
}

/// Bare, `CloudSyncState`-free background sync used by the Android headless Worker: pull, then
/// push when the dirty queue is non-empty. Calls the pull/push pipeline directly (the managed
/// wrappers that add revoke-enforcement/heartbeat/status aren't available in a headless
/// process). Best-effort — a transient network error surfaces as `Err` and the Worker retries.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) async fn run_background_sync(
    conn: Arc<Mutex<Connection>>,
    backend: &Arc<dyn CloudBackend>,
    session: &AuthSession,
    device_id: &str,
) -> Result<()> {
    pull::pull(conn.clone(), backend, session, device_id).await?;
    if dirty_nonempty(&conn)? {
        push::push(conn.clone(), backend, session, device_id).await?;
    }
    Ok(())
}

/// Whether the dirty queue has any pending buckets (the zero-duration case of
/// [`CloudSyncState::dirty_quiescent`](super::runtime::CloudSyncState::dirty_quiescent),
/// without needing the whole struct).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn dirty_nonempty(conn: &Arc<Mutex<Connection>>) -> Result<bool> {
    let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
    let count: i64 =
        guard.query_row("SELECT COUNT(*) FROM sync_dirty_buckets", [], |r| r.get(0))?;
    Ok(count > 0)
}
