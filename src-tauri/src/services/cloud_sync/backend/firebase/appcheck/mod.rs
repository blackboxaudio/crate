//! Firebase App Check token minting for **mobile** clients (#139).
//!
//! App Check cryptographically attests that a request comes from our genuine app on a genuine
//! device (Apple **App Attest** on iOS, **Play Integrity** on Android), raising the bar from
//! "needs a valid token" to "needs a valid token from an attested app instance". The desktop
//! app is a distributed binary with no workable provider, so App Check is **mobile-only**: on
//! desktop [`for_platform`] returns `None` and the Firebase backend attaches no header.
//!
//! We keep the repo's no-Firebase-SDK posture: the OS mints the raw attestation, and the
//! **token exchange happens in Rust** against `firebaseappcheck.googleapis.com` (see [`rest`]).
//! Each provider returns an [`AppCheckToken`]; [`AppCheckState`] caches and refreshes it on the
//! same skew schedule as the auth bearer token, and the backend stamps the resulting
//! `X-Firebase-AppCheck` header onto every Firestore / Storage / Identity-Toolkit request.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::config::CloudConfig;

mod debug;
pub(crate) mod rest;

#[cfg(target_os = "ios")]
mod app_attest;
#[cfg(target_os = "android")]
mod play_integrity;

/// A minted App Check token plus its absolute expiry (derived from the exchange `ttl`).
#[derive(Clone, Debug)]
pub(crate) struct AppCheckToken {
    pub token: String,
    pub expires_at: SystemTime,
}

/// Pluggable App Check attestation source — exactly one impl is selected per target OS.
/// `&self` (not `&mut`) so an impl can cache its long-lived material (e.g. the App Attest
/// key id + server artifact) behind its own interior mutability.
#[async_trait]
pub(crate) trait AppCheckProvider: Send + Sync {
    /// Run one full exchange against `firebaseappcheck.googleapis.com` and return a fresh token.
    async fn fetch_token(&self) -> Result<AppCheckToken>;
    /// Short label for diagnostics: `"debug"` | `"app_attest"` | `"play_integrity"`.
    fn kind(&self) -> &'static str;
}

/// Refresh a little before expiry, mirroring the auth bearer-token skew.
const APPCHECK_SKEW: Duration = Duration::from_secs(5 * 60);

/// App Check token cache + the provider that mints it. One per backend, shared via `Arc` so
/// every facet (manifest/blobs/devices/auth) reuses a single in-flight token.
pub(crate) struct AppCheckState {
    provider: Arc<dyn AppCheckProvider>,
    /// `std::sync::Mutex` is correct here: the guard is always dropped before the `.await`
    /// that mints, so it is never held across a suspension point.
    cached: Mutex<Option<AppCheckToken>>,
}

impl AppCheckState {
    pub(crate) fn new(provider: Arc<dyn AppCheckProvider>) -> Self {
        Self {
            provider,
            cached: Mutex::new(None),
        }
    }

    /// Return a valid App Check token, minting a new one when the cache is empty or within
    /// [`APPCHECK_SKEW`] of expiry. A token-expiry race at worst mints twice — harmless, since
    /// both tokens are individually valid and the last writer wins.
    pub(crate) async fn ensure_fresh(&self) -> Result<String> {
        // Fast path: decide under the lock, then DROP the guard before any await.
        {
            let cached = self.cached.lock().map_err(|_| CrateError::LockPoisoned)?;
            if let Some(tok) = cached.as_ref() {
                let still_fresh = tok
                    .expires_at
                    .duration_since(SystemTime::now())
                    .map(|left| left > APPCHECK_SKEW)
                    .unwrap_or(false);
                if still_fresh {
                    return Ok(tok.token.clone());
                }
            }
        }
        // Slow path: mint with NO lock held (the provider hits the network), then store.
        let minted = self.provider.fetch_token().await?;
        log::debug!(
            "cloud_sync: minted App Check token via {} provider",
            self.provider.kind()
        );
        let token = minted.token.clone();
        *self.cached.lock().map_err(|_| CrateError::LockPoisoned)? = Some(minted);
        Ok(token)
    }
}

/// Select the App Check provider for the current platform, or `None` when App Check is
/// inactive (desktop, or no Firebase App ID configured). Mirrors the per-`target_os` selection
/// in [`crate::db::key_provider`] — gated on the OS, not the Cargo feature, so a bare
/// `cargo check`/`cargo test` still compiles.
pub(crate) fn for_platform(
    client: reqwest::Client,
    config: &CloudConfig,
) -> Option<Arc<dyn AppCheckProvider>> {
    // Every exchange is scoped to a registered Firebase App ID; without one, no App Check.
    let app_id = config.firebase_app_id()?;

    // Dev/CI escape hatch on every platform: a configured debug token wins over native
    // attestation, letting a simulator/desktop build exercise the real exchange endpoint.
    if let Some(token) = config
        .appcheck_debug_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let provider: Arc<dyn AppCheckProvider> =
            Arc::new(debug::DebugProvider::new(client, config, app_id, token));
        return Some(provider);
    }

    #[cfg(target_os = "ios")]
    {
        let provider: Arc<dyn AppCheckProvider> = Arc::new(app_attest::AppAttestProvider::new(
            client,
            config,
            app_id.to_string(),
        ));
        Some(provider)
    }
    #[cfg(target_os = "android")]
    {
        let provider: Arc<dyn AppCheckProvider> = Arc::new(
            play_integrity::PlayIntegrityProvider::new(client, config, app_id.to_string()),
        );
        Some(provider)
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop: App Check is mobile-only. Without a debug token (handled above) there is no
        // provider, so the backend attaches no header — unchanged desktop behavior.
        let _ = (client, app_id);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned provider that hands out a fixed token with a controllable expiry and counts
    /// how many times it was asked to mint.
    struct FakeProvider {
        token: String,
        ttl: Duration,
        mints: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl AppCheckProvider for FakeProvider {
        async fn fetch_token(&self) -> Result<AppCheckToken> {
            self.mints
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(AppCheckToken {
                token: self.token.clone(),
                expires_at: SystemTime::now() + self.ttl,
            })
        }
        fn kind(&self) -> &'static str {
            "fake"
        }
    }

    #[tokio::test]
    async fn ensure_fresh_caches_until_skew() {
        let provider = Arc::new(FakeProvider {
            token: "tok".into(),
            ttl: Duration::from_secs(3600), // well beyond the 5-min skew
            mints: std::sync::atomic::AtomicUsize::new(0),
        });
        let state = AppCheckState::new(provider.clone());

        assert_eq!(state.ensure_fresh().await.unwrap(), "tok");
        assert_eq!(state.ensure_fresh().await.unwrap(), "tok");
        // Second call served from cache — only one mint.
        assert_eq!(provider.mints.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ensure_fresh_remints_when_within_skew() {
        let provider = Arc::new(FakeProvider {
            token: "tok".into(),
            ttl: Duration::from_secs(60), // inside the 5-min skew → always considered stale
            mints: std::sync::atomic::AtomicUsize::new(0),
        });
        let state = AppCheckState::new(provider.clone());

        state.ensure_fresh().await.unwrap();
        state.ensure_fresh().await.unwrap();
        // Cached token is already within skew of expiry, so each call re-mints.
        assert_eq!(provider.mints.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    /// Build a minimal valid [`CloudConfig`] (the 5 core fields) plus an optional trailing JSON
    /// fragment for the App Check keys under test.
    fn test_config(extra: &str) -> CloudConfig {
        let json = format!(
            r#"{{"project_id":"p","web_api_key":"k","storage_bucket":"b","oauth_client_id":"cid","oauth_client_secret":"secret"{extra}}}"#
        );
        CloudConfig::from_json(json.as_bytes()).expect("parse")
    }

    #[test]
    fn for_platform_none_without_app_id() {
        // No Firebase app id configured → App Check inactive on every platform.
        assert!(for_platform(reqwest::Client::new(), &test_config("")).is_none());
    }

    #[test]
    fn for_platform_debug_provider_when_token_set() {
        // App id + debug token → the debug provider on every platform (incl. these host tests).
        let cfg = test_config(
            r#","firebase_ios_app_id":"1:1:ios:x","appcheck_debug_token":"dbg""#,
        );
        let provider = for_platform(reqwest::Client::new(), &cfg).expect("provider");
        assert_eq!(provider.kind(), "debug");
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    #[test]
    fn for_platform_none_on_desktop_with_app_id_but_no_debug_token() {
        // A Firebase app id alone must NOT activate App Check on desktop — only a debug token
        // does. (On iOS/Android the same config would select the native provider.)
        let cfg = test_config(r#","firebase_ios_app_id":"1:1:ios:x""#);
        assert!(for_platform(reqwest::Client::new(), &cfg).is_none());
    }

    /// Manual end-to-end probe (hits the network; ignored by default). Exchanges the configured
    /// `appcheck_debug_token` for a real App Check token against Firebase, isolating the App Check
    /// path from the rest of cloud sync — the cheapest way to confirm the config + the exchange
    /// endpoint + the `X-Goog-Api-Key` auth, with no device or attestation.
    ///
    /// Prereqs: register a debug token in the Firebase console, then set `appcheck_debug_token`
    /// **and** `firebase_ios_app_id` in `src-tauri/cloud_sync.config.json`. Run from `src-tauri/`:
    ///
    /// ```text
    /// cargo test appcheck::tests::debug_token_probe -- --ignored --nocapture
    /// ```
    /// PASS prints the token length + expiry. FAIL panics with the Firebase error — a `403` means
    /// the token isn't registered (or the app id is wrong); if it specifically rejects the API key,
    /// that's the one protocol detail to flip (`X-Goog-Api-Key` → `?key=`).
    #[tokio::test]
    #[ignore = "network; needs a registered debug token in cloud_sync.config.json"]
    async fn debug_token_probe() {
        use crate::services::cloud_sync::config::load_cloud_config;

        let config = load_cloud_config(None)
            .expect("read cloud_sync.config.json")
            .expect("cloud_sync.config.json present with the 5 core fields");
        let app_id = config
            .firebase_app_id()
            .expect("set firebase_ios_app_id in cloud_sync.config.json")
            .to_string();
        let debug_token = config
            .appcheck_debug_token
            .clone()
            .filter(|t| !t.trim().is_empty())
            .expect("set appcheck_debug_token in cloud_sync.config.json");

        let provider =
            super::debug::DebugProvider::new(reqwest::Client::new(), &config, &app_id, &debug_token);
        match provider.fetch_token().await {
            Ok(token) => println!(
                "\n✅ App Check debug exchange OK — token length {}, expires_at {:?}\n",
                token.token.len(),
                token.expires_at
            ),
            Err(e) => panic!("\n❌ App Check debug exchange FAILED: {e}\n"),
        }
    }
}
