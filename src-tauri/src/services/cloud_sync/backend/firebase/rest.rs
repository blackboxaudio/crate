//! Shared REST plumbing for the Firebase backend.
//!
//! All Firestore docs in this backend store their payload as a SINGLE JSON string
//! field (`{ fields: { json: { stringValue: "<serde_json>" } } }`). The types already
//! derive `Serialize`/`Deserialize` (including `SystemTime`), CAS is document-level
//! (via `updateTime`) so the field shape is irrelevant, and this sidesteps Firestore's
//! typed-value mapping entirely — including the awkward `/` in track-shard names.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::{CrateError, Result};

pub(crate) use crate::services::cloud_sync::percent_encode;

/// Build the Firestore `fields` map that stores `value` as a single JSON string field.
pub(crate) fn json_fields<T: Serialize>(value: &T) -> Result<Value> {
    let encoded = serde_json::to_string(value)
        .map_err(|e| CrateError::CloudSync(format!("serialize doc: {e}")))?;
    Ok(json!({ "json": { "stringValue": encoded } }))
}

/// Build a full Firestore document body (`{ fields: { ... } }`) for a PATCH write.
pub(crate) fn json_field_doc<T: Serialize>(value: &T) -> Result<Value> {
    let fields = json_fields(value)?;
    Ok(json!({ "fields": fields }))
}

/// Parse a Firestore document whose payload is a single JSON string field.
pub(crate) fn parse_json_field<T: DeserializeOwned>(doc: &Value) -> Result<T> {
    let raw = doc
        .get("fields")
        .and_then(|f| f.get("json"))
        .and_then(|j| j.get("stringValue"))
        .and_then(|s| s.as_str())
        .ok_or_else(|| CrateError::CloudSync("Firestore doc missing `json` field".into()))?;
    serde_json::from_str(raw).map_err(|e| CrateError::CloudSync(format!("parse doc json: {e}")))
}

/// A Firestore document's server `updateTime` (used as the CAS token).
pub(crate) fn update_time(doc: &Value) -> Option<String> {
    doc.get("updateTime")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Map a non-success Firestore/Storage HTTP response to a `CrateError::CloudSyncHttp`.
/// Transience is decided by status via `CrateError::is_transient` (429/5xx → the runtime
/// shows `Offline` and retries; anything else is a hard error). (CAS-conflict detection
/// is done explicitly at the manifest commit site; blob 404 → `CloudSyncBlobNotFound` at
/// the blob site.)
pub(crate) async fn http_error(context: &str, resp: reqwest::Response) -> CrateError {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    CrateError::CloudSyncHttp {
        context: context.to_string(),
        status,
        code: sanitize_error_code(&body),
    }
}

/// Extract a short, URL/key-free token from a Google error body. Prefers a token-style
/// `error.message` (identitytoolkit/securetoken: "INVALID_REFRESH_TOKEN"), then the JSON
/// `error.status` token (Firestore/Storage: "PERMISSION_DENIED"), else the message/body —
/// the fallbacks truncated and stripped of anything that could carry a URL or API key,
/// since this string reaches logs, IPC, and the UI.
pub(crate) fn sanitize_error_code(body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let error = parsed.as_ref().and_then(|v| v.get("error"));
    let message = error
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str());
    if let Some(msg) = message {
        // Auth endpoints put the machine token in `message` (possibly with a trailing
        // ": details" segment) rather than `status`.
        let token = msg.split(&[':', ' '][..]).next().unwrap_or(msg);
        if !token.is_empty()
            && token.len() <= 64
            && token
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            return token.to_string();
        }
    }
    if let Some(status) = error.and_then(|e| e.get("status")).and_then(|s| s.as_str()) {
        return status.to_string();
    }
    let message = message.unwrap_or(body);
    let stripped: String = message
        .split_whitespace()
        .filter(|word| {
            !word.contains("http://") && !word.contains("https://") && !word.contains("key=")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: String = stripped.chars().take(120).collect();
    if stripped.chars().count() > 120 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sanitize_error_code;

    #[test]
    fn prefers_firestore_status_token() {
        let body = r#"{"error":{"code":403,"message":"Missing or insufficient permissions.","status":"PERMISSION_DENIED"}}"#;
        assert_eq!(sanitize_error_code(body), "PERMISSION_DENIED");
    }

    #[test]
    fn prefers_auth_message_token() {
        let body = r#"{"error":{"code":400,"message":"INVALID_REFRESH_TOKEN","status":"INVALID_ARGUMENT"}}"#;
        assert_eq!(sanitize_error_code(body), "INVALID_REFRESH_TOKEN");
        let with_detail = r#"{"error":{"code":400,"message":"TOKEN_EXPIRED : detail text"}}"#;
        assert_eq!(sanitize_error_code(with_detail), "TOKEN_EXPIRED");
    }

    #[test]
    fn strips_urls_and_keys_from_fallback() {
        let body = r#"{"error":{"message":"request to https://firestore.googleapis.com/v1/x?key=SECRET123 failed badly"}}"#;
        let out = sanitize_error_code(body);
        assert!(!out.contains("http"), "{out}");
        assert!(!out.contains("SECRET123"), "{out}");
        assert!(out.contains("request"), "{out}");
    }

    #[test]
    fn truncates_long_plain_bodies() {
        let body = "y".repeat(500);
        let out = sanitize_error_code(&body);
        assert!(out.chars().count() <= 121); // 120 + ellipsis
    }
}

/// Map a `reqwest` transport error (from `.send().await`) to a `CrateError`. A connect/
/// timeout/request failure means no usable response came back → transient
/// `CloudSyncNetwork`; anything else falls back to a generic `CloudSync`. Use at every
/// `.send().await` site so a dropped connection surfaces as `Offline`, not `Error`.
///
/// The URL is stripped first: the Firebase auth endpoints carry the Web API key as a
/// `?key=…` query param, and reqwest's `Display` embeds the failing URL — so an
/// unsanitized error would leak that key into logs, IPC, and `last_error` (shown in the
/// UI). `without_url` preserves the error kind, so the transient-vs-hard split still holds.
pub(crate) fn send_error(context: &str, e: reqwest::Error) -> CrateError {
    let e = e.without_url();
    let msg = format!("{context}: {e}");
    if e.is_connect() || e.is_timeout() || e.is_request() {
        CrateError::CloudSyncNetwork(msg)
    } else {
        CrateError::CloudSync(msg)
    }
}
