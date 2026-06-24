//! iOS **App Attest** provider (#139).
//!
//! `DCAppAttestService` (DeviceCheck.framework) mints the raw attestation/assertion in the
//! Secure Enclave; the Firebase App Check **exchange happens here in Rust** (no Firebase SDK),
//! mirroring how the rest of this backend speaks REST. The flow:
//!
//! 1. **Key** — `generateKey` once, persisted in the Keychain (the private key never leaves the
//!    Secure Enclave; we store only Apple's opaque `keyId`).
//! 2. **Attest** (first time, or when the artifact is rejected) — fetch a server `challenge`,
//!    `attestKey(SHA256(challenge))`, then `:exchangeAppAttestAttestation` → an App Check token
//!    plus a server `artifact` we persist.
//! 3. **Assert** (steady state) — fetch a fresh `challenge`, `generateAssertion(SHA256(challenge))`,
//!    then `:exchangeAppAttestAssertion { artifact, challenge, assertion }` → an App Check token.
//!    No re-attestation, so this is cheap on the hot path.
//!
//! Requires iOS 14+ and a **physical device** — `DCAppAttestService.isSupported` is `false` in
//! the simulator. Dev/CI uses the debug provider instead (selected when `appcheck_debug_token`
//! is set), so this path is exercised only on device.
//!
//! ## objc2 / protocol reconciliation surface (validate on device)
//! This file can only be type-checked with `cargo check --target aarch64-apple-ios` and only be
//! behaviorally validated on a registered device. Two classes of detail are marked `RECONCILE`:
//! - **objc2 bindings** — exact method/helper names on `NSData`/`NSString`/`NSError` across the
//!   objc2-foundation 0.3 surface (e.g. `to_vec`, `with_bytes`).
//! - **encoding** — whether the App Attest `clientDataHash` is `SHA256(decoded challenge bytes)`
//!   (implemented here) vs. the raw challenge string, and standard vs. url-safe base64 for the
//!   `bytes` JSON fields. Centralized in [`client_data_hash`] / [`b64`] so a fix is one line.

use std::sync::Mutex;

use async_trait::async_trait;
use base64::Engine as _;
use block2::{DynBlock, RcBlock};
use objc2_device_check::DCAppAttestService;
use objc2_foundation::{NSData, NSError, NSString};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::oneshot;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::config::CloudConfig;

use super::{rest, AppCheckProvider, AppCheckToken};

/// Keychain coordinates for the App Attest material (separate service from the SQLCipher key).
const KEYCHAIN_SERVICE: &str = "audio.bbx.crate.appcheck";
const KEYCHAIN_KEY_ID_ACCOUNT: &str = "app_attest_key_id";
const KEYCHAIN_ARTIFACT_ACCOUNT: &str = "app_attest_artifact";

/// The single App Attest challenge endpoint (used for both the attestation and assertion paths).
const CHALLENGE_VERB: &str = "generateAppAttestChallenge";

pub(crate) struct AppAttestProvider {
    client: reqwest::Client,
    project_id: String,
    app_id: String,
    web_api_key: String,
}

impl AppAttestProvider {
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

    /// POST an App Check exchange/challenge request carrying the project Web API key, returning
    /// the parsed JSON body on success (or a transient-aware error).
    async fn post(&self, verb: &str, body: serde_json::Value, context: &'static str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.endpoint(verb))
            .header(rest::API_KEY_HEADER, &self.web_api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| rest::send_error(context, e))?;
        if !resp.status().is_success() {
            return Err(rest::http_error(context, resp).await);
        }
        resp.json()
            .await
            .map_err(|e| CrateError::CloudSync(format!("{context} decode: {e}")))
    }

    /// Request a fresh single-use challenge from Firebase.
    async fn challenge(&self) -> Result<String> {
        let body = self
            .post(CHALLENGE_VERB, serde_json::json!({}), "appcheck app-attest challenge")
            .await?;
        body.get("challenge")
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| CrateError::CloudSync("appcheck challenge response missing `challenge`".into()))
    }

    /// First-time / recovery path: attest the key against a fresh challenge, exchange the
    /// attestation for a token, and persist the returned artifact for future assertions.
    async fn attest(&self, key_id: &str) -> Result<AppCheckToken> {
        let challenge = self.challenge().await?;
        let hash = client_data_hash(&challenge)?;
        let key_id_owned = key_id.to_string();
        let attestation = bridge_data("app attest attestKey", move |block| unsafe {
            let service = DCAppAttestService::sharedService();
            let key_ns = NSString::from_str(&key_id_owned);
            let hash_ns = NSData::with_bytes(&hash); // RECONCILE: NSData constructor name
            service.attestKey_clientDataHash_completionHandler(&key_ns, &hash_ns, block);
        })
        .await?;

        let body = serde_json::json!({
            "attestationStatement": b64(&attestation),
            "challenge": challenge,
            "keyId": key_id, // Apple's keyId is already base64; echo it through.
            "limitedUse": false,
        });
        let parsed: AttestationResponse = serde_json::from_value(
            self.post("exchangeAppAttestAttestation", body, "appcheck app-attest attestation")
                .await?,
        )
        .map_err(|e| CrateError::CloudSync(format!("appcheck attestation decode: {e}")))?;

        keychain_store(KEYCHAIN_ARTIFACT_ACCOUNT, &parsed.artifact)?;
        Ok(parsed.app_check_token.into_token())
    }

    /// Steady-state path: assert against a fresh challenge using the stored artifact.
    async fn assert(&self, key_id: &str, artifact: &str) -> Result<AppCheckToken> {
        let challenge = self.challenge().await?;
        let hash = client_data_hash(&challenge)?;
        let key_id_owned = key_id.to_string();
        let assertion = bridge_data("app attest generateAssertion", move |block| unsafe {
            let service = DCAppAttestService::sharedService();
            let key_ns = NSString::from_str(&key_id_owned);
            let hash_ns = NSData::with_bytes(&hash); // RECONCILE: NSData constructor name
            service.generateAssertion_clientDataHash_completionHandler(&key_ns, &hash_ns, block);
        })
        .await?;

        let body = serde_json::json!({
            "artifact": artifact,
            "challenge": challenge,
            "assertion": b64(&assertion),
            "limitedUse": false,
        });
        rest::parse_app_check_token(
            "appcheck app-attest assertion",
            self.client
                .post(self.endpoint("exchangeAppAttestAssertion"))
                .header(rest::API_KEY_HEADER, &self.web_api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| rest::send_error("appcheck app-attest assertion", e))?,
        )
        .await
    }

    /// Load the persisted App Attest key id, generating + persisting one on first run.
    async fn ensure_key(&self) -> Result<String> {
        if let Some(key_id) = keychain_load(KEYCHAIN_KEY_ID_ACCOUNT)? {
            return Ok(key_id);
        }
        if !app_attest_supported() {
            return Err(CrateError::CloudSync(
                "App Attest is not supported on this device".into(),
            ));
        }
        let key_id = bridge_string("app attest generateKey", |block| unsafe {
            DCAppAttestService::sharedService().generateKeyWithCompletionHandler(block);
        })
        .await?;
        keychain_store(KEYCHAIN_KEY_ID_ACCOUNT, &key_id)?;
        Ok(key_id)
    }
}

#[async_trait]
impl AppCheckProvider for AppAttestProvider {
    fn kind(&self) -> &'static str {
        "app_attest"
    }

    async fn fetch_token(&self) -> Result<AppCheckToken> {
        let key_id = self.ensure_key().await?;
        match keychain_load(KEYCHAIN_ARTIFACT_ACCOUNT)? {
            Some(artifact) => match self.assert(&key_id, &artifact).await {
                Ok(token) => Ok(token),
                // A network blip shouldn't burn a re-attestation; surface it as transient.
                Err(e) if e.is_transient() => Err(e),
                // A hard failure means the artifact was likely rotated/invalidated server-side —
                // re-attest once to recover.
                Err(e) => {
                    log::warn!("cloud_sync: App Attest assertion failed ({e}); re-attesting");
                    self.attest(&key_id).await
                }
            },
            None => self.attest(&key_id).await,
        }
    }
}

/// `:exchangeAppAttestAttestation` response — an App Check token plus the server artifact that
/// subsequent assertions reuse.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttestationResponse {
    app_check_token: rest::AppCheckTokenResponse,
    artifact: String,
}

// ---------------------------------------------------------------------------
// Encoding decisions — centralized so an on-device fix is a one-liner (see module doc).
// ---------------------------------------------------------------------------

/// The App Attest `clientDataHash`: SHA-256 over the challenge bytes. The server returns the
/// challenge base64-encoded; we decode it first and hash the raw bytes.
///
/// RECONCILE on device: if attestation is rejected, the alternative is `Sha256(challenge.as_bytes())`
/// (hashing the raw base64 string). Swap the `decode_b64` line for `challenge.as_bytes()` if so.
fn client_data_hash(challenge: &str) -> Result<[u8; 32]> {
    let bytes = decode_b64(challenge)?;
    Ok(Sha256::digest(bytes).into())
}

/// Standard base64 for the `bytes` JSON fields (proto3 JSON default). RECONCILE: switch to
/// `URL_SAFE` if the App Check backend rejects standard-alphabet statements.
fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Permissively decode a server-provided base64 string (standard or url-safe, padded or not).
fn decode_b64(s: &str) -> Result<Vec<u8>> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    STANDARD
        .decode(s)
        .or_else(|_| URL_SAFE.decode(s))
        .or_else(|_| STANDARD_NO_PAD.decode(s))
        .or_else(|_| URL_SAFE_NO_PAD.decode(s))
        .map_err(|e| CrateError::CloudSync(format!("appcheck challenge not base64: {e}")))
}

// ---------------------------------------------------------------------------
// objc2 bridging — completion-handler calls turned into awaitable futures.
// ---------------------------------------------------------------------------

/// Upper bound on a native App Attest completion handler. If a callback never fires (a framework
/// misbehavior or a bridging bug in this not-yet-device-validated code), the mint degrades to a
/// logged error and the request goes out without the header — never a stalled sync.
const NATIVE_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

fn app_attest_supported() -> bool {
    // SAFETY: `sharedService` returns the process-wide singleton; `isSupported` is a plain getter.
    unsafe { DCAppAttestService::sharedService().isSupported() }
}

/// Bridge a `…completionHandler:` call whose result is `NSData` to async.
///
/// All objc2 values (the service, the block, the args) are created and dropped **inside the inner
/// scope, before the `.await`** — so no `!Send` value crosses the suspension point and the
/// resulting future stays `Send` (required by the `#[async_trait]` bound). The framework copies
/// the escaping completion block during the call, so dropping our `RcBlock` early is safe; the
/// block extracts the bytes on the callback queue and sends only an owned `Vec<u8>` across.
async fn bridge_data(
    context: &'static str,
    invoke: impl FnOnce(&DynBlock<dyn Fn(*mut NSData, *mut NSError)>),
) -> Result<Vec<u8>> {
    let (tx, rx) = oneshot::channel::<Result<Vec<u8>>>();
    {
        let tx = Mutex::new(Some(tx));
        let block = RcBlock::new(move |data: *mut NSData, err: *mut NSError| {
            // SAFETY: pointers are valid for the duration of the completion callback.
            let result = unsafe { data_result(context, data, err) };
            if let Ok(mut slot) = tx.lock() {
                if let Some(tx) = slot.take() {
                    let _ = tx.send(result);
                }
            }
        });
        invoke(&block);
    }
    match tokio::time::timeout(NATIVE_CALL_TIMEOUT, rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(CrateError::CloudSync(format!(
            "{context}: completion handler dropped"
        ))),
        Err(_) => Err(CrateError::CloudSync(format!(
            "{context}: timed out after {NATIVE_CALL_TIMEOUT:?}"
        ))),
    }
}

/// Bridge a `…completionHandler:` call whose result is `NSString` (i.e. `generateKey`) to async.
async fn bridge_string(
    context: &'static str,
    invoke: impl FnOnce(&DynBlock<dyn Fn(*mut NSString, *mut NSError)>),
) -> Result<String> {
    let (tx, rx) = oneshot::channel::<Result<String>>();
    {
        let tx = Mutex::new(Some(tx));
        let block = RcBlock::new(move |s: *mut NSString, err: *mut NSError| {
            // SAFETY: pointers are valid for the duration of the completion callback.
            let result = unsafe { string_result(context, s, err) };
            if let Ok(mut slot) = tx.lock() {
                if let Some(tx) = slot.take() {
                    let _ = tx.send(result);
                }
            }
        });
        invoke(&block);
    }
    match tokio::time::timeout(NATIVE_CALL_TIMEOUT, rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(CrateError::CloudSync(format!(
            "{context}: completion handler dropped"
        ))),
        Err(_) => Err(CrateError::CloudSync(format!(
            "{context}: timed out after {NATIVE_CALL_TIMEOUT:?}"
        ))),
    }
}

/// # Safety
/// `data`/`err` are the completion-handler out-params: at most one is non-null.
unsafe fn data_result(context: &str, data: *mut NSData, err: *mut NSError) -> Result<Vec<u8>> {
    match data.as_ref() {
        Some(data) => Ok(data.to_vec()), // RECONCILE: NSData → bytes accessor name
        None => Err(ns_error(context, err)),
    }
}

/// # Safety
/// `s`/`err` are the completion-handler out-params: at most one is non-null.
unsafe fn string_result(context: &str, s: *mut NSString, err: *mut NSError) -> Result<String> {
    match s.as_ref() {
        Some(s) => Ok(s.to_string()),
        None => Err(ns_error(context, err)),
    }
}

/// # Safety
/// `err` is a completion-handler `NSError` out-param (may be null).
unsafe fn ns_error(context: &str, err: *mut NSError) -> CrateError {
    let detail = err
        .as_ref()
        .map(|e| e.localizedDescription().to_string())
        .unwrap_or_else(|| "unknown error".to_string());
    CrateError::CloudSync(format!("{context}: {detail}"))
}

// ---------------------------------------------------------------------------
// Keychain persistence (mirrors db::key_provider's iOS pattern).
// ---------------------------------------------------------------------------

fn keychain_load(account: &str) -> Result<Option<String>> {
    use security_framework::passwords::get_generic_password;
    use security_framework_sys::base::errSecItemNotFound;

    match get_generic_password(KEYCHAIN_SERVICE, account) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|e| CrateError::CloudSync(format!("appcheck keychain value not UTF-8: {e}"))),
        Err(e) if e.code() == errSecItemNotFound => Ok(None),
        Err(e) => Err(CrateError::CloudSync(format!(
            "appcheck keychain read failed (OSStatus {}): {e}",
            e.code()
        ))),
    }
}

fn keychain_store(account: &str, value: &str) -> Result<()> {
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::passwords::{set_generic_password_options, PasswordOptions};

    let access_control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleAfterFirstUnlockThisDeviceOnly),
        0,
    )
    .map_err(|e| {
        CrateError::CloudSync(format!(
            "appcheck keychain access control failed (OSStatus {}): {e}",
            e.code()
        ))
    })?;

    let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, account);
    options.set_access_control(access_control);
    set_generic_password_options(value.as_bytes(), options).map_err(|e| {
        CrateError::CloudSync(format!(
            "appcheck keychain write failed (OSStatus {}): {e}",
            e.code()
        ))
    })
}
