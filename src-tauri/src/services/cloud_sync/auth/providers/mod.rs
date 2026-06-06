//! Enabled identity providers. V1 = Google only; Apple/Microsoft are commented stubs
//! that slot in as one new file + one line here.

pub mod google;
// pub mod apple;       // future: Apple Service ID + form_post + nonce-signing
// pub mod microsoft;   // future: tenant config

use std::sync::Arc;

use super::provider::IdentityProvider;

/// The providers enabled in this build, in display order.
pub fn enabled_providers() -> Vec<Arc<dyn IdentityProvider>> {
    vec![
        Arc::new(google::GoogleProvider::new()),
        // Arc::new(apple::AppleProvider::new()),
        // Arc::new(microsoft::MicrosoftProvider::new()),
    ]
}

/// Look up an enabled provider by its [`IdentityProvider::id`].
pub fn provider_by_id(id: &str) -> Option<Arc<dyn IdentityProvider>> {
    enabled_providers().into_iter().find(|p| p.id() == id)
}
