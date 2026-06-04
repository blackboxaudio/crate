//! Firebase Authentication via the Identity Toolkit + Secure Token REST APIs, keyed
//! by the project's Web API key. Crate never sees the user's password — the provider
//! ID token comes from the loopback OAuth flow ([`crate::services::cloud_sync::auth`]).

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::{AuthSession, ProfileInfo};
use crate::services::cloud_sync::backend::AuthBackend;

use super::{rest, FirebaseInner};

pub(crate) struct FirebaseAuth {
    inner: Arc<FirebaseInner>,
}

impl FirebaseAuth {
    pub(crate) fn new(inner: Arc<FirebaseInner>) -> Self {
        Self { inner }
    }
}

/// `accounts:signInWithIdp` response (camelCase).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignInResponse {
    id_token: String,
    refresh_token: String,
    expires_in: String,
    local_id: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    photo_url: Option<String>,
}

/// `securetoken` refresh response (snake_case — note this differs from signInWithIdp).
#[derive(Deserialize)]
struct RefreshResponse {
    id_token: String,
    refresh_token: String,
    expires_in: String,
    user_id: String,
}

fn expires_at(expires_in: &str) -> SystemTime {
    let secs: u64 = expires_in.parse().unwrap_or(3600);
    SystemTime::now() + Duration::from_secs(secs)
}

#[async_trait]
impl AuthBackend for FirebaseAuth {
    async fn sign_in_with_idp(&self, provider_id: &str, id_token: &str) -> Result<AuthSession> {
        let url = format!(
            "https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp?key={}",
            self.inner.config.web_api_key
        );
        let body = json!({
            "postBody": format!("id_token={id_token}&providerId={provider_id}"),
            "requestUri": "http://127.0.0.1",
            "returnIdpCredential": true,
            "returnSecureToken": true,
        });
        let resp = self
            .inner
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| rest::send_error("signInWithIdp request", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CrateError::CloudSyncAuth(format!(
                "signInWithIdp HTTP {status}: {text}"
            )));
        }
        let p: SignInResponse = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSyncAuth(format!("signInWithIdp decode: {e}")))?;
        Ok(AuthSession {
            uid: p.local_id,
            access_token: p.id_token,
            refresh_token: p.refresh_token,
            access_token_expires_at: expires_at(&p.expires_in),
            email: p.email,
            display_name: p.display_name,
            photo_url: p.photo_url,
        })
    }

    async fn refresh(&self, refresh_token: &str) -> Result<AuthSession> {
        let url = format!(
            "https://securetoken.googleapis.com/v1/token?key={}",
            self.inner.config.web_api_key
        );
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        let resp = self
            .inner
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| rest::send_error("token refresh request", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CrateError::CloudSyncAuth(format!(
                "token refresh HTTP {status}: {text}"
            )));
        }
        let p: RefreshResponse = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSyncAuth(format!("token refresh decode: {e}")))?;
        // securetoken omits profile fields; the orchestrator repatches them from cache.
        Ok(AuthSession {
            uid: p.user_id,
            access_token: p.id_token,
            refresh_token: p.refresh_token,
            access_token_expires_at: expires_at(&p.expires_in),
            email: None,
            display_name: None,
            photo_url: None,
        })
    }

    async fn sign_out(&self, _session: &AuthSession) -> Result<()> {
        // Firebase ID tokens are stateless JWTs; "sign out" is dropping the refresh
        // token locally (handled by `auth::token_store`). Nothing to revoke server-side
        // for the desktop flow in v1.
        Ok(())
    }

    async fn lookup_profile(&self, session: &AuthSession) -> Result<ProfileInfo> {
        let url = format!(
            "https://identitytoolkit.googleapis.com/v1/accounts:lookup?key={}",
            self.inner.config.web_api_key
        );
        let body = json!({ "idToken": session.access_token });
        let resp = self
            .inner
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| rest::send_error("accounts:lookup request", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CrateError::CloudSyncAuth(format!(
                "accounts:lookup HTTP {status}: {text}"
            )));
        }
        let body: LookupResponse = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSyncAuth(format!("accounts:lookup decode: {e}")))?;
        let user =
            body.users.into_iter().next().ok_or_else(|| {
                CrateError::CloudSyncAuth("accounts:lookup returned no users".into())
            })?;
        Ok(ProfileInfo {
            email: user.email,
            display_name: user.display_name,
            photo_url: user.photo_url,
        })
    }
}

/// `accounts:lookup` response — only the fields we care about for the profile card.
#[derive(Deserialize)]
struct LookupResponse {
    #[serde(default)]
    users: Vec<LookupUser>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupUser {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    photo_url: Option<String>,
}
