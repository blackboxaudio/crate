//! Google identity provider (OpenID Connect over OAuth 2.0 + PKCE).

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::auth::provider::{AuthRequestParams, IdentityProvider};
use crate::services::cloud_sync::percent_encode;

pub struct GoogleProvider;

impl GoogleProvider {
    pub fn new() -> Self {
        Self
    }
}

impl IdentityProvider for GoogleProvider {
    fn id(&self) -> &'static str {
        "google"
    }
    fn display_name(&self) -> &'static str {
        "Google"
    }
    fn firebase_provider_id(&self) -> &'static str {
        "google.com"
    }
    fn scopes(&self) -> &'static [&'static str] {
        &["openid", "email", "profile"]
    }
    fn token_endpoint(&self) -> &'static str {
        "https://oauth2.googleapis.com/token"
    }

    fn authorization_url(&self, p: &AuthRequestParams) -> String {
        let scope = self.scopes().join(" ");
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth\
?response_type=code\
&client_id={client_id}\
&redirect_uri={redirect}\
&scope={scope}\
&code_challenge={challenge}\
&code_challenge_method=S256\
&state={state}",
            client_id = percent_encode(p.client_id),
            redirect = percent_encode(p.redirect_uri),
            scope = percent_encode(&scope),
            challenge = percent_encode(p.code_challenge),
            state = percent_encode(p.state),
        )
    }

    fn extract_id_token(&self, resp: &serde_json::Value) -> Result<String> {
        resp.get("id_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| CrateError::CloudSyncAuth("token response missing id_token".into()))
    }
}
