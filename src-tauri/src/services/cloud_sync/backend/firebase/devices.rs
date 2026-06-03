//! Firestore devices collection (`users/{uid}/devices/{deviceId}`). Each device doc
//! stores its [`DeviceRecord`] as a single JSON string field (see [`super::rest`]).

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde_json::Value;

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
        // PATCH creates-or-replaces the doc with the given fields.
        let url = self.device_url(&s.uid, &device.device_id);
        let resp = self
            .inner
            .client
            .patch(&url)
            .bearer_auth(&s.access_token)
            .json(&rest::json_field_doc(device)?)
            .send()
            .await
            .map_err(|e| CrateError::CloudSync(format!("device upsert request: {e}")))?;
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
            .map_err(|e| CrateError::CloudSync(format!("device list request: {e}")))?;
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
                if let Ok(rec) = rest::parse_json_field::<DeviceRecord>(doc) {
                    out.push(rec);
                }
            }
        }
        Ok(out)
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
            .map_err(|e| CrateError::CloudSync(format!("device remove request: {e}")))?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(rest::http_error("device remove", resp).await);
        }
        Ok(())
    }
}
