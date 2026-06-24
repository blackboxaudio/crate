//! Android **Play Integrity** provider (#139).
//!
//! The native Play Integrity API (Kotlin, `AppCheckPlayIntegrity.kt`) mints a device/app integrity
//! token for a server-issued nonce; the Firebase App Check **exchange happens here in Rust** (no
//! Firebase SDK). The flow:
//!
//! 1. `:generatePlayIntegrityChallenge` → a single-use `challenge` used as the Play Integrity nonce.
//! 2. JNI → `AppCheckPlayIntegrity.requestToken(context, cloudProjectNumber, nonce)` → a raw
//!    integrity token (blocking; run on a `spawn_blocking` task).
//! 3. `:exchangePlayIntegrityToken { playIntegrityToken }` → a Firebase App Check token (the server
//!    verifies the nonce embedded in the token matches the challenge it issued).
//!
//! Requires Play services on the device and the app linked in the Google Play Console. Dev/CI uses
//! the debug provider instead (selected when `appcheck_debug_token` is set), so this path runs only
//! on a Play-registered device/emulator.
//!
//! ## Reconciliation surface (validate on device — see also `AppCheckPlayIntegrity.kt`)
//! This file only type-checks with `cargo check --target aarch64-linux-android` and behaves only on
//! a Play-registered device. `RECONCILE` marks the JNI plumbing (jni 0.21 call shapes; that
//! `ndk_context` is populated by the Tauri/wry runtime) and the Kotlin method signature.

use async_trait::async_trait;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::config::CloudConfig;

use super::{rest, AppCheckProvider, AppCheckToken};

/// Fully-qualified JNI class name of the Kotlin helper.
const HELPER_CLASS: &str = "com/bbx_audio/crateapp/AppCheckPlayIntegrity";

pub(crate) struct PlayIntegrityProvider {
    client: reqwest::Client,
    project_id: String,
    app_id: String,
    web_api_key: String,
}

impl PlayIntegrityProvider {
    pub(crate) fn new(client: reqwest::Client, config: &CloudConfig, app_id: String) -> Self {
        Self {
            client,
            project_id: config.project_id.clone(),
            app_id,
            web_api_key: config.web_api_key.clone(),
        }
    }

    fn endpoint(&self, verb: &str) -> String {
        rest::endpoint(&self.project_id, &self.app_id, verb)
    }

    /// Request a fresh single-use challenge from Firebase (used as the Play Integrity nonce).
    async fn challenge(&self) -> Result<String> {
        let resp = self
            .client
            .post(self.endpoint("generatePlayIntegrityChallenge"))
            .header(rest::API_KEY_HEADER, &self.web_api_key)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| rest::send_error("appcheck play-integrity challenge", e))?;
        if !resp.status().is_success() {
            return Err(rest::http_error("appcheck play-integrity challenge", resp).await);
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| {
            CrateError::CloudSync(format!("appcheck play-integrity challenge decode: {e}"))
        })?;
        body.get("challenge")
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                CrateError::CloudSync(
                    "appcheck play-integrity challenge response missing `challenge`".into(),
                )
            })
    }

    /// Exchange a raw Play Integrity token for a Firebase App Check token.
    async fn exchange(&self, integrity_token: &str) -> Result<AppCheckToken> {
        let body = serde_json::json!({ "playIntegrityToken": integrity_token, "limitedUse": false });
        let resp = self
            .client
            .post(self.endpoint("exchangePlayIntegrityToken"))
            .header(rest::API_KEY_HEADER, &self.web_api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| rest::send_error("appcheck play-integrity exchange", e))?;
        rest::parse_app_check_token("appcheck play-integrity exchange", resp).await
    }
}

#[async_trait]
impl AppCheckProvider for PlayIntegrityProvider {
    fn kind(&self) -> &'static str {
        "play_integrity"
    }

    async fn fetch_token(&self) -> Result<AppCheckToken> {
        let cloud_project_number = parse_project_number(&self.app_id)?;
        let nonce = self.challenge().await?;
        let integrity_token = integrity_token(cloud_project_number, nonce).await?;
        self.exchange(&integrity_token).await
    }
}

/// The GCP project **number** Play Integrity needs (an `i64`), parsed from the Firebase App ID
/// `1:<projectNumber>:android:<hash>` so we don't carry a separate config value.
fn parse_project_number(app_id: &str) -> Result<i64> {
    app_id
        .split(':')
        .nth(1)
        .and_then(|n| n.parse::<i64>().ok())
        .ok_or_else(|| {
            CrateError::CloudSync(format!(
                "could not parse a project number from Firebase app id '{app_id}'"
            ))
        })
}

/// Run the blocking JNI call off the async runtime.
async fn integrity_token(cloud_project_number: i64, nonce: String) -> Result<String> {
    tokio::task::spawn_blocking(move || native_integrity_token(cloud_project_number, &nonce))
        .await
        .map_err(|e| CrateError::CloudSync(format!("appcheck play-integrity task failed: {e}")))?
}

/// Call `AppCheckPlayIntegrity.requestToken(context, cloudProjectNumber, nonce)` over JNI and
/// return the raw integrity token. Blocking; invoked via [`integrity_token`].
fn native_integrity_token(cloud_project_number: i64, nonce: &str) -> Result<String> {
    use jni::objects::{JObject, JString, JValue};

    // The Tauri/wry runtime populates the global Android context (JVM + Activity). RECONCILE if
    // unset at call time (would mean the runtime hasn't initialized it on this thread/path).
    let ctx = ndk_context::android_context();

    // SAFETY: `vm()`/`context()` are valid process-lifetime handles owned by the runtime.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| CrateError::CloudSync(format!("appcheck JNI JavaVM: {e}")))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| CrateError::CloudSync(format!("appcheck JNI attach: {e}")))?;
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };

    let nonce = env
        .new_string(nonce)
        .map_err(|e| CrateError::CloudSync(format!("appcheck JNI nonce string: {e}")))?;

    // RECONCILE: signature `(Landroid/content/Context;JLjava/lang/String;)Ljava/lang/String;`.
    let result = env
        .call_static_method(
            HELPER_CLASS,
            "requestToken",
            "(Landroid/content/Context;JLjava/lang/String;)Ljava/lang/String;",
            &[
                JValue::Object(&context),
                JValue::Long(cloud_project_number),
                JValue::Object(&nonce),
            ],
        )
        .map_err(|e| CrateError::CloudSync(format!("Play Integrity requestToken failed: {e}")))?;

    let token_obj = result
        .l()
        .map_err(|e| CrateError::CloudSync(format!("appcheck JNI token object: {e}")))?;
    let token: String = env
        .get_string(&JString::from(token_obj))
        .map_err(|e| CrateError::CloudSync(format!("appcheck JNI token decode: {e}")))?
        .into();
    Ok(token)
}
