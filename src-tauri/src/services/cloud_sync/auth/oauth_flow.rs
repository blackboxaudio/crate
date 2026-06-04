//! Provider-agnostic loopback OAuth 2.0 + PKCE.
//!
//! Binds a one-shot `axum` listener on `127.0.0.1:0` (same pattern as `proxy.rs`),
//! opens the system browser to the provider's consent screen, captures the redirected
//! `?code=`, and exchanges it (with the PKCE verifier) for the provider's ID token.
//! Crate never sees the user's password — credentials are entered on the provider's
//! domain in the browser; only a one-time authorization code returns to the loopback.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use sha2::{Digest, Sha256};
use tokio::sync::oneshot;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::auth::provider::{AuthRequestParams, IdentityProvider};

/// How long to wait for the user to finish the browser sign-in before giving up.
const FLOW_TIMEOUT: Duration = Duration::from_secs(300);

/// Run the loopback OAuth + PKCE flow for `provider`, returning the provider ID token.
///
/// `open_url` is invoked once with the authorization URL; the command layer routes it
/// to the system browser via the opener plugin (kept as a closure so this module stays
/// free of Tauri types).
pub async fn run_loopback_flow(
    provider: &dyn IdentityProvider,
    client_id: &str,
    client_secret: &str,
    open_url: impl FnOnce(&str) -> Result<()> + Send,
) -> Result<String> {
    // 1. PKCE pair + CSRF state.
    let verifier = random_b64url(32);
    let challenge = code_challenge(&verifier);
    let state = random_b64url(16);

    // 2. One-shot loopback listener on an OS-assigned port (mirrors proxy.rs).
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| CrateError::CloudSyncAuth(format!("bind loopback: {e}")))?;
    let port = std_listener
        .local_addr()
        .map_err(|e| CrateError::CloudSyncAuth(format!("loopback addr: {e}")))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let (tx, rx) = oneshot::channel::<CallbackResult>();
    let shared = Arc::new(CallbackState {
        tx: tokio::sync::Mutex::new(Some(tx)),
        expected_state: state.clone(),
    });

    std_listener
        .set_nonblocking(true)
        .map_err(|e| CrateError::CloudSyncAuth(format!("loopback nonblocking: {e}")))?;
    let listener = tokio::net::TcpListener::from_std(std_listener)
        .map_err(|e| CrateError::CloudSyncAuth(format!("loopback tokio: {e}")))?;

    // Catch-all route so any redirect target (e.g. with an unexpected path) still hits
    // the handler — Google sometimes appends an empty path the router would 404 on.
    let router = axum::Router::new()
        .route("/", axum::routing::get(callback_handler))
        .fallback(axum::routing::get(callback_handler))
        .with_state(shared);
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    // 3. Open the consent screen.
    let auth_url = provider.authorization_url(&AuthRequestParams {
        client_id,
        redirect_uri: &redirect_uri,
        code_challenge: &challenge,
        state: &state,
    });
    if let Err(e) = open_url(&auth_url) {
        server.abort();
        return Err(e);
    }

    // 4. Await the redirect (with a timeout).
    let code = match tokio::time::timeout(FLOW_TIMEOUT, rx).await {
        Ok(Ok(CallbackResult::Code(code))) => code,
        Ok(Ok(CallbackResult::Error(msg))) => {
            server.abort();
            return Err(CrateError::CloudSyncAuth(format!(
                "authorization failed: {msg}"
            )));
        }
        Ok(Err(_)) => {
            server.abort();
            return Err(CrateError::CloudSyncAuth("sign-in canceled".into()));
        }
        Err(_) => {
            server.abort();
            return Err(CrateError::CloudSyncAuth("sign-in timed out".into()));
        }
    };
    server.abort();

    // 5. Exchange code + verifier for the provider ID token.
    exchange_code(
        provider,
        client_id,
        client_secret,
        &redirect_uri,
        &code,
        &verifier,
    )
    .await
}

struct CallbackState {
    tx: tokio::sync::Mutex<Option<oneshot::Sender<CallbackResult>>>,
    expected_state: String,
}

enum CallbackResult {
    Code(String),
    Error(String),
}

async fn callback_handler(
    axum::extract::State(state): axum::extract::State<Arc<CallbackState>>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> axum::response::Html<&'static str> {
    let params = parse_query(query.as_deref().unwrap_or(""));
    let result = if let Some(err) = params.get("error") {
        CallbackResult::Error(err.clone())
    } else if params.get("state").map(String::as_str) != Some(state.expected_state.as_str()) {
        CallbackResult::Error("state mismatch".into())
    } else if let Some(code) = params.get("code") {
        CallbackResult::Code(code.clone())
    } else {
        CallbackResult::Error("missing authorization code".into())
    };

    if let Some(tx) = state.tx.lock().await.take() {
        let _ = tx.send(result);
    }
    axum::response::Html(CALLBACK_HTML)
}

const CALLBACK_HTML: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>Crate</title></head>\
<body style=\"font-family:system-ui,sans-serif;padding:3rem;text-align:center\">\
<h2>Signed in to Crate</h2>\
<p>You can close this tab and return to the app.</p></body></html>";

async fn exchange_code(
    provider: &dyn IdentityProvider,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| CrateError::CloudSyncAuth(format!("token exchange client build: {e}")))?;
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ];
    let resp = client
        .post(provider.token_endpoint())
        .form(&params)
        .send()
        .await
        .map_err(|e| CrateError::CloudSyncAuth(format!("token exchange request: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(CrateError::CloudSyncAuth(format!(
            "token exchange HTTP {status}: {text}"
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| CrateError::CloudSyncAuth(format!("token exchange decode: {e}")))?;
    provider.extract_id_token(&body)
}

// --- PKCE + query helpers -------------------------------------------------------

/// `n_bytes` of randomness, base64url-no-pad (a valid PKCE verifier / state token).
fn random_b64url(n_bytes: usize) -> String {
    let bytes: Vec<u8> = (0..n_bytes).map(|_| rand::random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// PKCE S256 challenge: base64url-no-pad(SHA-256(verifier)).
fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Parse an `x=1&y=2` query string into a decoded map.
fn parse_query(q: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if !k.is_empty() {
            map.insert(percent_decode(k), percent_decode(v));
        }
    }
    map
}

/// Minimal `application/x-www-form-urlencoded` percent-decoder (`+` → space).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push((h << 4) | l);
                    i += 3;
                }
                _ => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
