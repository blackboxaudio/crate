//! iOS key provider: the SQLCipher key lives in the Keychain as a generic
//! password, stored with `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
//!
//! That accessibility class means the key is readable after the first unlock
//! following a boot (so background launches and sync work), but is never synced
//! to iCloud Keychain and never restored to another device. A new device
//! re-syncs its library from the cloud, so a non-portable key is acceptable.
//!
//! Single-app Keychain access needs no entitlement and no `keychain-access-groups`.
//! The service/account are fixed constants — the dev/staging/prod builds have
//! distinct bundle ids, so the OS already isolates their Keychains.

use std::path::Path;

use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::passwords::{
    generic_password, set_generic_password_options, PasswordOptions,
};

use super::{generate_key, KeyProvider};
use crate::error::{CrateError, Result};

/// Keychain service + account under which the key is stored.
const KEYCHAIN_SERVICE: &str = "com.bbx-audio.crate.db";
const KEYCHAIN_ACCOUNT: &str = "sqlcipher-key";

/// `errSecItemNotFound` — returned by a Keychain read when the item does not exist
/// yet (i.e. first launch). Distinct from any other failure, which must NOT be
/// treated as a first launch.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// Retrieves (or, on first launch, generates and stores) the key from the iOS Keychain.
pub struct IosKeychainProvider;

impl KeyProvider for IosKeychainProvider {
    fn get_or_create(&self, _app_data_dir: &Path) -> Result<String> {
        match generic_password(PasswordOptions::new_generic_password(
            KEYCHAIN_SERVICE,
            KEYCHAIN_ACCOUNT,
        )) {
            Ok(bytes) => {
                let key = String::from_utf8(bytes)
                    .map_err(|e| {
                        CrateError::KeyStorage(format!("keychain key is not valid UTF-8: {e}"))
                    })?
                    .trim()
                    .to_string();
                if key.is_empty() {
                    // A present-but-empty item is corruption, not a first launch.
                    // Regenerating here would orphan the existing encrypted database.
                    return Err(CrateError::KeyStorage(
                        "keychain returned an empty key; refusing to overwrite".to_string(),
                    ));
                }
                Ok(key)
            }
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {
                // First launch: generate a key and store it with the chosen accessibility.
                let key = generate_key();
                store_key(&key)?;
                Ok(key)
            }
            // Any other failure (e.g. the device is still locked before its first unlock)
            // must propagate — never fall through to regeneration.
            Err(e) => Err(CrateError::KeyStorage(format!("keychain read failed: {e}"))),
        }
    }
}

/// Store `key` as a generic password, accessible after first unlock and bound to this device.
fn store_key(key: &str) -> Result<()> {
    let access = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleAfterFirstUnlockThisDeviceOnly),
        0, // no SecAccessControlCreateFlags: non-interactive read at launch, no biometric gate
    )
    .map_err(|e| CrateError::KeyStorage(format!("failed to build keychain access control: {e}")))?;

    let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
    options.set_access_control(access);

    // Argument order is (password bytes, options-by-value).
    set_generic_password_options(key.as_bytes(), options)
        .map_err(|e| CrateError::KeyStorage(format!("failed to store key in keychain: {e}")))
}
