//! Shared REST plumbing for the Firebase backend.
//!
//! All Firestore docs in this backend store their payload as a SINGLE JSON string
//! field (`{ fields: { json: { stringValue: "<serde_json>" } } }`). The types already
//! derive `Serialize`/`Deserialize` (including `SystemTime`), CAS is document-level
//! (via `updateTime`) so the field shape is irrelevant, and this sidesteps Firestore's
//! typed-value mapping entirely — including the awkward `/` in track-shard names.

use reqwest::StatusCode;
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

/// Map a non-success Firestore/Storage HTTP response to a `CrateError`. A `429` or `5xx`
/// is transient (`CloudSyncNetwork` → the runtime shows `Offline` and retries); anything
/// else is a generic `CloudSync`. (CAS-conflict detection is done explicitly at the
/// manifest commit site; blob 404 → `CloudSyncBlobNotFound` at the blob site.)
pub(crate) async fn http_error(context: &str, resp: reqwest::Response) -> CrateError {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let msg = format!("{context}: HTTP {status}: {body}");
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        CrateError::CloudSyncNetwork(msg)
    } else {
        CrateError::CloudSync(msg)
    }
}

/// Map a `reqwest` transport error (from `.send().await`) to a `CrateError`. A connect/
/// timeout/request failure means no usable response came back → transient
/// `CloudSyncNetwork`; anything else falls back to a generic `CloudSync`. Use at every
/// `.send().await` site so a dropped connection surfaces as `Offline`, not `Error`.
pub(crate) fn send_error(context: &str, e: reqwest::Error) -> CrateError {
    let msg = format!("{context}: {e}");
    if e.is_connect() || e.is_timeout() || e.is_request() {
        CrateError::CloudSyncNetwork(msg)
    } else {
        CrateError::CloudSync(msg)
    }
}
