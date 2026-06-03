//! Refresh-token storage in the OS keychain via the `keyring` crate (Keychain on
//! macOS, Credential Manager on Windows, Secret Service on Linux).
//!
//! The Firebase refresh token is the ONLY persisted credential. The short-lived ID
//! token stays in memory and is re-minted on demand via `securetoken`.

use keyring::Entry;

use crate::error::{CrateError, Result};

const SERVICE: &str = "com.bbx.crate.cloudsync";
const ACCOUNT: &str = "firebase-refresh-token";

fn entry() -> Result<Entry> {
    Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| CrateError::CloudSyncAuth(format!("keychain open: {e}")))
}

/// Persist (or replace) the Firebase refresh token.
pub fn store_refresh_token(token: &str) -> Result<()> {
    entry()?
        .set_password(token)
        .map_err(|e| CrateError::CloudSyncAuth(format!("keychain store: {e}")))
}

/// Load the Firebase refresh token, or `None` if not signed in on this device.
pub fn load_refresh_token() -> Result<Option<String>> {
    match entry()?.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(CrateError::CloudSyncAuth(format!("keychain load: {e}"))),
    }
}

/// Delete the stored refresh token (sign-out). A missing entry is not an error.
pub fn clear_refresh_token() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CrateError::CloudSyncAuth(format!("keychain clear: {e}"))),
    }
}
