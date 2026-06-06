//! Firestore devices collection (`users/{uid}/devices/{deviceId}`). Each device doc
//! stores its [`DeviceRecord`] as a single JSON string field (see [`super::rest`]), plus
//! a separate top-level boolean `revoked` field. The heartbeat and the revoke each write
//! with a disjoint `updateMask` (`json` vs. `revoked`), so an in-flight heartbeat can't
//! clear a concurrent revoke (and vice-versa).

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::{AuthSession, DeviceRecord};
use crate::services::cloud_sync::backend::DeviceRegistry;

use super::{rest, FirebaseInner};

pub(crate) struct FirebaseDevices {
    inner: Arc<FirebaseInner>,
}

impl FirebaseDevices {
    pub(crate) fn new(inner: Arc<FirebaseInner>) -> Self {
        Self { inner }
    }

    fn device_url(&self, uid: &str, device_id: &str) -> String {
        format!(
            "{}/users/{uid}/devices/{}",
            self.inner.firestore_base(),
            rest::percent_encode(device_id)
        )
    }
}

#[async_trait]
impl DeviceRegistry for FirebaseDevices {
    async fn upsert(&self, s: &AuthSession, device: &DeviceRecord) -> Result<()> {
        // Heartbeat: PATCH only the `json` blob field (disjoint update mask), so it never
        // clobbers a concurrent `revoked` flag. Creates the doc if absent.
        let url = format!(
            "{}?updateMask.fieldPaths=json",
            self.device_url(&s.uid, &device.device_id)
        );
        let resp = self
            .inner
            .client
            .patch(&url)
            .bearer_auth(&s.access_token)
            .json(&rest::json_field_doc(device)?)
            .send()
            .await
            .map_err(|e| rest::send_error("device upsert request", e))?;
        if !resp.status().is_success() {
            return Err(rest::http_error("device upsert", resp).await);
        }
        Ok(())
    }

    async fn list(&self, s: &AuthSession) -> Result<Vec<DeviceRecord>> {
        let url = format!(
            "{}/users/{}/devices?pageSize=300",
            self.inner.firestore_base(),
            s.uid
        );
        let resp = self
            .inner
            .client
            .get(&url)
            .bearer_auth(&s.access_token)
            .send()
            .await
            .map_err(|e| rest::send_error("device list request", e))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !resp.status().is_success() {
            return Err(rest::http_error("device list", resp).await);
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSync(format!("device list decode: {e}")))?;

        let mut out = Vec::new();
        if let Some(docs) = body.get("documents").and_then(|d| d.as_array()) {
            for doc in docs {
                if let Ok(rec) = parse_device(doc) {
                    out.push(rec);
                }
            }
        }
        Ok(out)
    }

    async fn get(&self, s: &AuthSession, device_id: &str) -> Result<Option<DeviceRecord>> {
        let url = self.device_url(&s.uid, device_id);
        let resp = self
            .inner
            .client
            .get(&url)
            .bearer_auth(&s.access_token)
            .send()
            .await
            .map_err(|e| rest::send_error("device get request", e))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(rest::http_error("device get", resp).await);
        }
        let doc: Value = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSync(format!("device get decode: {e}")))?;
        Ok(Some(parse_device(&doc)?))
    }

    async fn set_revoked(&self, s: &AuthSession, device_id: &str, revoked: bool) -> Result<()> {
        // Write ONLY the top-level `revoked` field (disjoint update mask), so this never
        // clobbers a concurrent heartbeat's `json` blob.
        let url = format!(
            "{}?updateMask.fieldPaths=revoked",
            self.device_url(&s.uid, device_id)
        );
        let body = json!({ "fields": { "revoked": { "booleanValue": revoked } } });
        let resp = self
            .inner
            .client
            .patch(&url)
            .bearer_auth(&s.access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| rest::send_error("device set_revoked request", e))?;
        if !resp.status().is_success() {
            return Err(rest::http_error("device set_revoked", resp).await);
        }
        Ok(())
    }

    async fn remove(&self, s: &AuthSession, device_id: &str) -> Result<()> {
        let url = self.device_url(&s.uid, device_id);
        let resp = self
            .inner
            .client
            .delete(&url)
            .bearer_auth(&s.access_token)
            .send()
            .await
            .map_err(|e| rest::send_error("device remove request", e))?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(rest::http_error("device remove", resp).await);
        }
        Ok(())
    }
}

/// Parse a device doc: the `json` string field → [`DeviceRecord`], then overlay the
/// separate top-level `revoked` boolean (absent → `false`).
fn parse_device(doc: &Value) -> Result<DeviceRecord> {
    let mut rec: DeviceRecord = rest::parse_json_field(doc)?;
    rec.revoked = doc
        .get("fields")
        .and_then(|f| f.get("revoked"))
        .and_then(|r| r.get("booleanValue"))
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    Ok(rec)
}
