//! Cloud-sync authentication orchestration.
//!
//! Runs the provider-agnostic loopback flow, hands the resulting provider ID token to
//! the backend's `signInWithIdp`, and persists the Firebase refresh token in the OS
//! keychain. Crate never sees the user's password. The short-lived ID token lives in
//! memory and is refreshed within ~5 min of expiry.

pub mod keychain;
pub mod oauth_flow;
pub mod provider;
pub mod providers;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, OptionalExtension};

use crate::error::{CrateError, Result};

use super::backend::types::AuthSession;
use super::backend::CloudBackend;
use super::config::CloudConfig;
use provider::IdentityProvider;

/// Refresh the access token when within this window of its expiry.
const REFRESH_SKEW: Duration = Duration::from_secs(5 * 60);

/// Run the full sign-in: loopback OAuth for `provider` → Firebase `signInWithIdp` →
/// persist the refresh token + cache profile fields. Returns the live session.
pub async fn sign_in(
    backend: &Arc<dyn CloudBackend>,
    config: &CloudConfig,
    provider: &dyn IdentityProvider,
    conn: Arc<Mutex<Connection>>,
    open_url: impl FnOnce(&str) -> Result<()> + Send,
) -> Result<AuthSession> {
    let id_token = oauth_flow::run_loopback_flow(
        provider,
        &config.oauth_client_id,
        &config.oauth_client_secret,
        open_url,
    )
    .await?;

    let session = backend
        .auth()
        .sign_in_with_idp(provider.firebase_provider_id(), &id_token)
        .await?;

    // Keychain writes are blocking and can prompt the OS (e.g. macOS Keychain access
    // dialog), so push them off the async runtime.
    let refresh = session.refresh_token.clone();
    tokio::task::spawn_blocking(move || keychain::store_refresh_token(&refresh))
        .await
        .map_err(|e| CrateError::CloudSyncAuth(format!("keychain task join: {e}")))??;

    persist_profile(&conn, &session)?;
    Ok(session)
}

/// Sign out: best-effort backend sign-out, then clear the keychain refresh token.
pub async fn sign_out(
    backend: &Arc<dyn CloudBackend>,
    session: Option<&AuthSession>,
) -> Result<()> {
    if let Some(s) = session {
        let _ = backend.auth().sign_out(s).await;
    }
    keychain::clear_refresh_token()?;
    Ok(())
}

/// Restore a session from the stored refresh token (startup / before a push). Returns
/// `None` when not signed in. Repatches the cached email/display_name that the
/// `securetoken` refresh response omits, and persists any rotated refresh token.
pub async fn current_session(
    backend: &Arc<dyn CloudBackend>,
    conn: Arc<Mutex<Connection>>,
) -> Result<Option<AuthSession>> {
    let Some(refresh_token) = keychain::load_refresh_token()? else {
        return Ok(None);
    };
    let mut session = backend.auth().refresh(&refresh_token).await?;
    let (email, display_name, photo_url) = read_profile(&conn)?;
    session.email = session.email.or(email);
    session.display_name = session.display_name.or(display_name);
    session.photo_url = session.photo_url.or(photo_url);
    keychain::store_refresh_token(&session.refresh_token)?;
    Ok(Some(session))
}

/// Return `session` as-is if its access token is still fresh; otherwise re-mint it
/// from the stored refresh token. Falls back to the original session if a refresh
/// isn't possible (e.g. signed out underneath us).
pub async fn ensure_fresh(
    backend: &Arc<dyn CloudBackend>,
    conn: Arc<Mutex<Connection>>,
    session: AuthSession,
) -> Result<AuthSession> {
    let expiring_soon = session
        .access_token_expires_at
        .duration_since(SystemTime::now())
        .map(|left| left <= REFRESH_SKEW)
        .unwrap_or(true);
    if expiring_soon {
        if let Some(fresh) = current_session(backend, conn).await? {
            return Ok(fresh);
        }
    }
    Ok(session)
}

fn persist_profile(conn: &Arc<Mutex<Connection>>, s: &AuthSession) -> Result<()> {
    let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
    write_state(&guard, "cloud_uid", &s.uid)?;
    write_state(&guard, "cloud_email", s.email.as_deref().unwrap_or(""))?;
    write_state(
        &guard,
        "cloud_display_name",
        s.display_name.as_deref().unwrap_or(""),
    )?;
    write_state(
        &guard,
        "cloud_photo_url",
        s.photo_url.as_deref().unwrap_or(""),
    )?;
    Ok(())
}

/// Update only the cached profile fields (email/display_name/photo_url) without
/// touching `cloud_uid` or any session state. Used to refresh the avatar after a
/// successful sync.
pub fn persist_profile_fields(
    conn: &Arc<Mutex<Connection>>,
    email: Option<&str>,
    display_name: Option<&str>,
    photo_url: Option<&str>,
) -> Result<()> {
    let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
    write_state(&guard, "cloud_email", email.unwrap_or(""))?;
    write_state(&guard, "cloud_display_name", display_name.unwrap_or(""))?;
    write_state(&guard, "cloud_photo_url", photo_url.unwrap_or(""))?;
    Ok(())
}

pub(crate) fn read_profile(
    conn: &Arc<Mutex<Connection>>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let guard = conn.lock().map_err(|_| CrateError::LockPoisoned)?;
    let email = read_state(&guard, "cloud_email")?.filter(|s| !s.is_empty());
    let display_name = read_state(&guard, "cloud_display_name")?.filter(|s| !s.is_empty());
    let photo_url = read_state(&guard, "cloud_photo_url")?.filter(|s| !s.is_empty());
    Ok((email, display_name, photo_url))
}

fn write_state(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

fn read_state(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}
