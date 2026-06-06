//! Cloud-sync configuration loading.
//!
//! The five values that point the app at its Firebase project live in a gitignored
//! `src-tauri/cloud_sync.config.json` (see the committed `cloud_sync.config.example.json`).
//! None are confidential secrets — a desktop OAuth client secret is non-confidential —
//! but they're kept out of git.
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
}

impl CloudConfig {
    /// Parse a config from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| CrateError::CloudSync(format!("invalid {CONFIG_FILE_NAME}: {e}")))
    }

    /// True only when every field is non-blank — a half-filled template counts as
    /// "not configured".
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
