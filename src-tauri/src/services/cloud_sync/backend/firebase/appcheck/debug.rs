//! The App Check **debug** provider: a pure-Rust exchange of a console-registered debug token
//! for an App Check token, with no native attestation. Selected on every platform when
//! `appcheck_debug_token` is configured (dev/CI only — it bypasses attestation), so a simulator
//! or desktop build can exercise the real exchange endpoint and the whole header funnel without
//! attestation hardware.

use async_trait::async_trait;

use crate::error::Result;
use crate::services::cloud_sync::config::CloudConfig;

use super::{rest, AppCheckProvider, AppCheckToken};

pub(crate) struct DebugProvider {
    client: reqwest::Client,
    /// Pre-built `…:exchangeDebugToken` endpoint for the configured project + app id.
    url: String,
    web_api_key: String,
    debug_token: String,
}

impl DebugProvider {
    pub(crate) fn new(
        client: reqwest::Client,
        config: &CloudConfig,
        app_id: &str,
        debug_token: &str,
    ) -> Self {
        Self {
            client,
            url: rest::endpoint(&config.project_id, app_id, "exchangeDebugToken"),
            web_api_key: config.web_api_key.clone(),
            debug_token: debug_token.to_string(),
        }
    }
}

#[async_trait]
impl AppCheckProvider for DebugProvider {
    fn kind(&self) -> &'static str {
        "debug"
    }

    async fn fetch_token(&self) -> Result<AppCheckToken> {
        // `limitedUse: false` — we want a standard, reusable session token for the header, not a
        // one-time-use token (which is for replay-sensitive operations we don't perform).
        let body = serde_json::json!({ "debugToken": self.debug_token, "limitedUse": false });
        let resp = self
            .client
            .post(&self.url)
            .header(rest::API_KEY_HEADER, &self.web_api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| rest::send_error("appcheck debug exchange", e))?;
        rest::parse_app_check_token("appcheck debug exchange", resp).await
    }
}
