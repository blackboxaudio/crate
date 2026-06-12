//! Android key provider: the SQLCipher key is held via the Android Keystore.
//!
//! `android-keyring` wraps the key with a non-exportable Keystore AES key and
//! stores the ciphertext in SharedPreferences (the standard envelope pattern —
//! the Keystore holds keys used to wrap/unwrap, not arbitrary retrievable
//! secrets). It runs entirely in Rust over JNI using the `ndk-context` that
//! Tauri Mobile initializes before `Database::new` runs inside `.setup()`.
//!
//! Like the iOS provider, the stored key is device-bound: it is not backed up or
//! migrated to another device, which matches the cloud-resyncable data model.
//!
//! NOTE: compile-checked in CI but not yet device-tested (there is no Android
//! project/device wired up yet — see M.002). If `android-keyring` proves
//! unreliable, this whole file can be swapped for a hand-rolled `jni` +
//! `ndk-context` envelope without touching the rest of the codebase.

use std::path::Path;
use std::sync::Once;

use keyring::{Entry, Error as KeyringError};

use super::{generate_key, KeyProvider};
use crate::error::{CrateError, Result};

/// Keystore service + user under which the key is stored.
const KEYSTORE_SERVICE: &str = "com.bbx-audio.crate.db";
const KEYSTORE_USER: &str = "sqlcipher-key";

/// Registers the android-keyring credential store exactly once per process.
static INIT: Once = Once::new();

/// Retrieves (or, on first launch, generates and stores) the key from the Android Keystore.
pub struct AndroidKeystoreProvider;

impl KeyProvider for AndroidKeystoreProvider {
    fn get_or_create(&self, _app_data_dir: &Path) -> Result<String> {
        ensure_credential_builder()?;

        let entry = Entry::new(KEYSTORE_SERVICE, KEYSTORE_USER)
            .map_err(|e| CrateError::KeyStorage(format!("failed to open keystore entry: {e}")))?;

        match entry.get_password() {
            Ok(key) => {
                let key = key.trim().to_string();
                if key.is_empty() {
                    // A present-but-empty item is corruption, not a first launch.
                    return Err(CrateError::KeyStorage(
                        "keystore returned an empty key; refusing to overwrite".to_string(),
                    ));
                }
                Ok(key)
            }
            Err(KeyringError::NoEntry) => {
                // First launch: generate a key and store it in the Keystore.
                let key = generate_key();
                entry.set_password(&key).map_err(|e| {
                    CrateError::KeyStorage(format!("failed to store key in keystore: {e}"))
                })?;
                Ok(key)
            }
            // Any other failure must propagate — never fall through to regeneration.
            Err(e) => Err(CrateError::KeyStorage(format!("keystore read failed: {e}"))),
        }
    }
}

/// Initialize the android-keyring credential builder once (idempotent).
///
/// Relies on the ndk-context that Tauri Mobile sets up before `Database::new`
/// runs. Since `Database::new` runs exactly once per process, the `Once` guard's
/// "only the first call sees the error" semantics are sufficient here.
fn ensure_credential_builder() -> Result<()> {
    let mut init_result: Result<()> = Ok(());
    INIT.call_once(|| {
        if let Err(e) = android_keyring::set_android_keyring_credential_builder() {
            init_result = Err(CrateError::KeyStorage(format!(
                "failed to initialize android keyring: {e}"
            )));
        }
    });
    init_result
}
