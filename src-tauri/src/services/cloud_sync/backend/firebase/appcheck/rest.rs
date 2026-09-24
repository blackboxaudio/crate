//! REST plumbing for the Firebase App Check token-exchange endpoints.
//!
//! Host `firebaseappcheck.googleapis.com`; every exchange is a POST to
//! `…/v1/projects/{project}/apps/{appId}:{verb}` authenticated by the project Web API key in
//! the `X-Goog-Api-Key` header — deliberately NOT a `?key=` query param, so the URL carries no
//! secret and the URL-stripping [`send_error`] can't leak it. The shared success payload is a
//! `{ token, ttl }` [`AppCheckTokenResponse`]; `ttl` is a protobuf Duration string (`"3600s"`).

use std::time::{Duration, SystemTime};

use serde::Deserialize;

use crate::error::{CrateError, Result};

use super::AppCheckToken;

// Reuse the parent backend's transient-aware, URL-stripping error mappers so an offline App
// Check mint surfaces as `Offline` (and retries), exactly like the Firestore/Storage calls.
pub(crate) use super::super::rest::{http_error, send_error};

const HOST: &str = "https://firebaseappcheck.googleapis.com/v1";

/// Header carrying the project Web API key on App Check exchange calls.
pub(crate) const API_KEY_HEADER: &str = "X-Goog-Api-Key";

/// Build an exchange endpoint URL, e.g.
/// `…/v1/projects/{project_id}/apps/{app_id}:exchangeDebugToken`. The project **id** is
/// accepted in place of the numeric project number.
pub(crate) fn endpoint(project_id: &str, app_id: &str, verb: &str) -> String {
    format!("{HOST}/projects/{project_id}/apps/{app_id}:{verb}")
}

/// The bare `{ token, ttl }` success payload returned by most exchange endpoints.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppCheckTokenResponse {
    pub token: String,
    pub ttl: String,
}

impl AppCheckTokenResponse {
    pub(crate) fn into_token(self) -> AppCheckToken {
        AppCheckToken {
            token: self.token,
            expires_at: expires_at(&self.ttl),
        }
    }
}

/// Decode a `{ token, ttl }` exchange response into an [`AppCheckToken`], routing a non-success
/// status through the shared transient-aware error path.
pub(crate) async fn parse_app_check_token(
    context: &str,
    resp: reqwest::Response,
) -> Result<AppCheckToken> {
    if !resp.status().is_success() {
        return Err(http_error(context, resp).await);
    }
    let parsed: AppCheckTokenResponse = resp
        .json()
        .await
        .map_err(|e| CrateError::CloudSync(format!("{context} decode: {e}")))?;
    Ok(parsed.into_token())
}

/// Convert a protobuf Duration string (`"3600s"`, possibly fractional `"1799.5s"`) into an
/// absolute expiry. Falls back to a conservative one hour on any parse failure.
pub(crate) fn expires_at(ttl: &str) -> SystemTime {
    let secs = ttl
        .trim()
        .strip_suffix('s')
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(3600.0);
    SystemTime::now() + Duration::from_secs_f64(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_builds_exchange_url() {
        assert_eq!(
            endpoint("proj", "1:2:ios:abc", "exchangeDebugToken"),
            "https://firebaseappcheck.googleapis.com/v1/projects/proj/apps/1:2:ios:abc:exchangeDebugToken"
        );
    }

    #[test]
    fn ttl_parses_integer_and_fractional() {
        let now = SystemTime::now();
        let secs = expires_at("3600s").duration_since(now).unwrap().as_secs();
        assert!((3590..=3601).contains(&secs), "got {secs}");

        let frac = expires_at("1799.5s").duration_since(now).unwrap().as_secs();
        assert!((1790..=1800).contains(&frac), "got {frac}");
    }

    #[test]
    fn ttl_falls_back_to_one_hour_on_garbage() {
        let now = SystemTime::now();
        let secs = expires_at("not-a-duration")
            .duration_since(now)
            .unwrap()
            .as_secs();
        assert!((3590..=3601).contains(&secs), "got {secs}");
    }
}
