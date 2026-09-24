//! Cloud-sync configuration loading.
//!
//! The five core values that point the app at its Firebase project live in a gitignored
//! `src-tauri/cloud_sync.config.json` (see the committed `cloud_sync.config.example.json`).
//! None are confidential secrets — a desktop OAuth client secret is non-confidential —
//! but they're kept out of git.
//!
//! Two optional mobile-only keys (`ios_oauth_client_id`, `android_oauth_client_id`) carry
//! the platform-specific Google OAuth client ids used by the native mobile sign-in flow
//! (public clients, no secret). They are absent on desktop and excluded from the
//! "is this config complete?" check, so a desktop config that omits them is still valid.
//!
//! Three further optional keys configure mobile Firebase App Check (#139): the
//! `firebase_ios_app_id` / `firebase_android_app_id` Firebase App IDs that App Check tokens
//! are minted against, and a dev/CI-only `appcheck_debug_token` that selects the debug
//! provider in place of native attestation. All three are likewise excluded from the
//! completeness check — their absence just leaves App Check inactive.
//!
//! Loading degrades gracefully: a missing or blank file means cloud sync is simply
//! unavailable, never a startup panic. A present-but-malformed file is surfaced as an
//! error so a typo doesn't silently disable sync.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CrateError, Result};

const CONFIG_FILE_NAME: &str = "cloud_sync.config.json";

/// Points the app at a Firebase project. Loaded from a gitignored JSON file; cloned
/// into the Firebase backend and the auth flow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudConfig {
    pub project_id: String,
    pub web_api_key: String,
    pub storage_bucket: String,
    pub oauth_client_id: String,
    pub oauth_client_secret: String,
    /// iOS Google OAuth client id (public client, no secret). Mobile-only; optional so
    /// existing desktop configs stay valid. `None` until the iOS client is provisioned (#49).
    #[serde(default)]
    pub ios_oauth_client_id: Option<String>,
    /// Android Google OAuth client id (public client, no secret). Mobile-only; optional.
    #[serde(default)]
    pub android_oauth_client_id: Option<String>,
    /// Firebase **App ID** for the iOS app (format `1:<projectNumber>:ios:<hash>`). Distinct
    /// from the OAuth client id and the bundle id; minted by registering the iOS app in the
    /// Firebase console. Required for App Check (App Attest) on iOS; `None` disables it.
    #[serde(default)]
    pub firebase_ios_app_id: Option<String>,
    /// Firebase **App ID** for the Android app (`1:<projectNumber>:android:<hash>`). Required
    /// for App Check (Play Integrity) on Android; `None` disables it.
    #[serde(default)]
    pub firebase_android_app_id: Option<String>,
    /// App Check **debug** token secret (dev/CI only). When set, the debug provider is used on
    /// every platform in place of native attestation, so a simulator/desktop build can exercise
    /// the real exchange endpoint. MUST be empty/absent in release config — it bypasses
    /// attestation. Optional; excluded from the completeness check.
    #[serde(default)]
    pub appcheck_debug_token: Option<String>,
}

impl CloudConfig {
    /// Parse a config from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| CrateError::CloudSync(format!("invalid {CONFIG_FILE_NAME}: {e}")))
    }

    /// True when the five core (desktop/shared) fields are non-blank — a half-filled template
    /// counts as "not configured". The optional mobile client ids are deliberately excluded:
    /// their absence makes mobile sign-in fail with a clear error, but must not disable cloud
    /// sync for a desktop config that legitimately omits them.
    fn is_complete(&self) -> bool {
        ![
            &self.project_id,
            &self.web_api_key,
            &self.storage_bucket,
            &self.oauth_client_id,
            &self.oauth_client_secret,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
    }

    /// The `(client_id, callback_scheme)` for native mobile sign-in on the current platform.
    ///
    /// iOS and Android each need their own Google OAuth client (public client, no secret). The
    /// OAuth callback uses the *reversed client id* scheme — Google's convention for iOS/Android
    /// clients. Errors when the platform's client id isn't configured, or on non-mobile targets.
    pub fn mobile_client(&self) -> Result<(String, String)> {
        #[cfg(target_os = "ios")]
        let id = self.ios_oauth_client_id.clone();
        #[cfg(target_os = "android")]
        let id = self.android_oauth_client_id.clone();
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let id: Option<String> = None;

        let id = id
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| CrateError::CloudSync("mobile OAuth client id not configured".into()))?;
        let scheme = reversed_client_id(&id);
        Ok((id, scheme))
    }

    /// The Firebase App ID used for App Check on the current platform: the iOS id on iOS, the
    /// Android id on Android. On other targets (a desktop dev build exercising the debug
    /// provider) it returns whichever id is configured, preferring iOS. `None` when no app id
    /// is configured — in which case App Check is simply inactive.
    pub fn firebase_app_id(&self) -> Option<&str> {
        #[cfg(target_os = "ios")]
        let id = self.firebase_ios_app_id.as_deref();
        #[cfg(target_os = "android")]
        let id = self.firebase_android_app_id.as_deref();
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let id = self
            .firebase_ios_app_id
            .as_deref()
            .or(self.firebase_android_app_id.as_deref());
        id
    }

    /// Compile-time fallback for release builds, which don't ship a config file. The five
    /// values are public client identifiers (security rests on PKCE + Firebase Auth +
    /// Security Rules), so baking them into the binary is expected. Injected via
    /// `GCLOUD_*` env vars at build time (see `.github/workflows/cd.release.yml`).
    /// Returns `None` unless all five are present and non-blank.
    fn from_compiled_env() -> Option<Self> {
        let config = CloudConfig {
            project_id: option_env!("GCLOUD_PROJECT_ID")
                .unwrap_or_default()
                .to_string(),
            web_api_key: option_env!("GCLOUD_WEB_API_KEY")
                .unwrap_or_default()
                .to_string(),
            storage_bucket: option_env!("GCLOUD_STORAGE_BUCKET")
                .unwrap_or_default()
                .to_string(),
            oauth_client_id: option_env!("GCLOUD_OAUTH_CLIENT_ID")
                .unwrap_or_default()
                .to_string(),
            oauth_client_secret: option_env!("GCLOUD_OAUTH_CLIENT_SECRET")
                .unwrap_or_default()
                .to_string(),
            ios_oauth_client_id: option_env!("GCLOUD_IOS_OAUTH_CLIENT_ID").map(str::to_string),
            android_oauth_client_id: option_env!("GCLOUD_ANDROID_OAUTH_CLIENT_ID")
                .map(str::to_string),
            firebase_ios_app_id: option_env!("GCLOUD_FIREBASE_IOS_APP_ID").map(str::to_string),
            firebase_android_app_id: option_env!("GCLOUD_FIREBASE_ANDROID_APP_ID")
                .map(str::to_string),
            appcheck_debug_token: option_env!("GCLOUD_APPCHECK_DEBUG_TOKEN").map(str::to_string),
        };
        config.is_complete().then_some(config)
    }
}

/// Load the cloud-sync config, or `None` when it's absent/blank (sync unavailable).
///
/// Search order:
/// 1. The current working directory — this is `src-tauri/` during `yarn dev`.
/// 2. `app_config_dir`, if provided — lets a packaged build drop the file alongside
///    the app's other config.
/// 3. Compile-time `GCLOUD_*` env vars baked in by the release workflow — the path
///    distributed builds take, since they ship no config file.
///
/// A missing file is not an error; a present-but-malformed file is.
pub fn load_cloud_config(app_config_dir: Option<&Path>) -> Result<Option<CloudConfig>> {
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from(CONFIG_FILE_NAME)];
    if let Some(dir) = app_config_dir {
        candidates.push(dir.join(CONFIG_FILE_NAME));
    }

    for path in candidates {
        match std::fs::read(&path) {
            Ok(bytes) => {
                let config = CloudConfig::from_json(&bytes)?;
                if !config.is_complete() {
                    log::warn!(
                        "cloud_sync: {} at {} has blank fields; cloud sync disabled",
                        CONFIG_FILE_NAME,
                        path.display()
                    );
                    return Ok(None);
                }
                log::info!("cloud_sync: loaded config from {}", path.display());
                return Ok(Some(config));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                log::warn!("cloud_sync: could not read {}: {e}", path.display());
                continue;
            }
        }
    }

    // Release builds ship no config file — fall back to values baked in at compile time.
    if let Some(config) = CloudConfig::from_compiled_env() {
        log::info!("cloud_sync: loaded config from compile-time env (GCLOUD_*)");
        return Ok(Some(config));
    }

    log::info!("cloud_sync: no {CONFIG_FILE_NAME} or compile-time config; cloud sync unavailable");
    Ok(None)
}

/// Derive a Google OAuth client's reversed-client-id URL scheme from its client id, e.g.
/// `123-abc.apps.googleusercontent.com` → `com.googleusercontent.apps.123-abc`. Idempotent if
/// the id is already missing the suffix (returns `com.googleusercontent.apps.<id>`).
fn reversed_client_id(client_id: &str) -> String {
    let head = client_id
        .strip_suffix(".apps.googleusercontent.com")
        .unwrap_or(client_id);
    format!("com.googleusercontent.apps.{head}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_client_id_strips_suffix() {
        assert_eq!(
            reversed_client_id("123-abc.apps.googleusercontent.com"),
            "com.googleusercontent.apps.123-abc"
        );
    }

    #[test]
    fn reversed_client_id_handles_bare_id() {
        // Idempotent-ish: an id without the suffix is just prefixed.
        assert_eq!(
            reversed_client_id("123-abc"),
            "com.googleusercontent.apps.123-abc"
        );
    }

    #[test]
    fn config_without_mobile_keys_deserializes_and_is_complete() {
        let json = br#"{
            "project_id": "p",
            "web_api_key": "k",
            "storage_bucket": "b",
            "oauth_client_id": "cid",
            "oauth_client_secret": "secret"
        }"#;
        let config = CloudConfig::from_json(json).expect("parse");
        assert!(config.is_complete());
        assert!(config.ios_oauth_client_id.is_none());
        assert!(config.android_oauth_client_id.is_none());
    }

    #[test]
    fn config_with_mobile_keys_populates_fields() {
        let json = br#"{
            "project_id": "p",
            "web_api_key": "k",
            "storage_bucket": "b",
            "oauth_client_id": "cid",
            "oauth_client_secret": "secret",
            "ios_oauth_client_id": "ios.apps.googleusercontent.com",
            "android_oauth_client_id": "android.apps.googleusercontent.com"
        }"#;
        let config = CloudConfig::from_json(json).expect("parse");
        assert!(config.is_complete());
        assert_eq!(
            config.ios_oauth_client_id.as_deref(),
            Some("ios.apps.googleusercontent.com")
        );
        assert_eq!(
            config.android_oauth_client_id.as_deref(),
            Some("android.apps.googleusercontent.com")
        );
    }
}
