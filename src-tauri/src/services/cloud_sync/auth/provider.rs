//! Identity-provider abstraction.
//!
//! V1 enables Google only ([`super::providers`]). Adding Apple or Microsoft later is a
//! new `IdentityProvider` impl + one line in `enabled_providers()` — the loopback
//! orchestrator ([`super::oauth_flow`]), the Firebase backend, and the frontend wiring
//! are all provider-agnostic.

use crate::error::Result;

/// PKCE + CSRF parameters for one authorization request.
pub struct AuthRequestParams<'a> {
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    /// base64url(SHA-256(verifier)) — the PKCE `code_challenge` (S256).
    pub code_challenge: &'a str,
    /// Opaque CSRF token echoed back on the redirect and verified.
    pub state: &'a str,
}

pub trait IdentityProvider: Send + Sync {
    /// Stable internal id (`"google"`).
    fn id(&self) -> &'static str;
    /// Human-readable name (`"Google"`).
    fn display_name(&self) -> &'static str;
    /// Firebase `signInWithIdp` provider id (`"google.com"`).
    fn firebase_provider_id(&self) -> &'static str;
    /// OAuth scopes to request.
    fn scopes(&self) -> &'static [&'static str];
    /// Provider token endpoint (authorization-code exchange).
    fn token_endpoint(&self) -> &'static str;
    /// Build the full authorization URL the system browser opens.
    fn authorization_url(&self, params: &AuthRequestParams) -> String;
    /// Pull the provider ID token out of the token-exchange JSON response.
    fn extract_id_token(&self, token_response: &serde_json::Value) -> Result<String>;
}
