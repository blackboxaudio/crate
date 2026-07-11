//! SQLCipher key provisioning, abstracted per platform.
//!
//! The encryption key for the SQLCipher database is sourced differently depending on the
//! target OS:
//!
//! * **Desktop** (macOS / Windows / Linux) — a 64-char hex key persisted to a `db.key`
//!   file in the app data dir (Unix `0o600`). Behavior is unchanged from the original
//!   inline implementation.
//! * **iOS** — stored in the Keychain via `security-framework`, with
//!   `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` accessibility (device-only and
//!   non-exportable; never synced to iCloud nor migrated to a new device).
//! * **Android** — wrapped by a non-exportable Android Keystore AES-GCM key via a JNI bridge
//!   to `CrateDbKey.kt` (#144); the wrapped blob lives in app-private SharedPreferences.
//!   Works on both the foreground (Tauri) and headless (WorkManager) paths — see
//!   `crate::android_context`. `android:allowBackup` is disabled so a device-transfer can't
//!   restore a blob whose Keystore wrapping key didn't travel with it.
//!
//! Provider selection is gated on `target_os` rather than the `desktop`/`mobile` Cargo
//! feature, because the correct secure store is fundamentally a per-OS decision. This also
//! keeps `db` compiling under a bare `cargo check`/`cargo test` (no platform feature
//! selected), where the file provider is used.

use std::path::Path;

use crate::error::{CrateError, Result};

/// Resolves the SQLCipher passphrase, retrieving the persisted key or generating and
/// storing a new one on first launch.
pub(crate) trait KeyProvider {
    fn get_or_create_key(&self) -> Result<String>;
}

/// Generate a fresh 64-char hex passphrase (two UUIDv4s, CSPRNG-backed via getrandom).
/// This is the same format the desktop file provider has always produced, so existing
/// SQLCipher databases keep opening unchanged.
#[cfg(not(target_os = "android"))]
fn generate_key() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    )
}

// ---------------------------------------------------------------------------
// Factory — exactly one definition is compiled per target OS.
// ---------------------------------------------------------------------------

/// Select the key provider for the current target OS.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub(crate) fn for_platform(app_data_dir: &Path) -> Box<dyn KeyProvider> {
    Box::new(FileKeyProvider::new(app_data_dir.to_path_buf()))
}

#[cfg(target_os = "ios")]
pub(crate) fn for_platform(_app_data_dir: &Path) -> Box<dyn KeyProvider> {
    Box::new(KeychainKeyProvider::new())
}

#[cfg(target_os = "android")]
pub(crate) fn for_platform(_app_data_dir: &Path) -> Box<dyn KeyProvider> {
    Box::new(AndroidKeystoreKeyProvider::new())
}

// ---------------------------------------------------------------------------
// Desktop: file-based provider (behavior unchanged).
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "ios", target_os = "android")))]
struct FileKeyProvider {
    app_data_dir: std::path::PathBuf,
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl FileKeyProvider {
    fn new(app_data_dir: std::path::PathBuf) -> Self {
        Self { app_data_dir }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl KeyProvider for FileKeyProvider {
    fn get_or_create_key(&self) -> Result<String> {
        let key_path = self.app_data_dir.join("db.key");

        if key_path.exists() {
            let key = std::fs::read_to_string(&key_path)
                .map_err(|e| CrateError::KeyStorage(format!("failed to read key file: {e}")))?;
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Ok(key);
            }
        }

        // First launch: generate a random key and persist it with restrictive permissions.
        let key = generate_key();
        write_key_file(&key_path, &key)?;
        Ok(key)
    }
}

/// Write the key to a file with restrictive permissions.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn write_key_file(path: &Path, key: &str) -> Result<()> {
    std::fs::write(path, key)
        .map_err(|e| CrateError::KeyStorage(format!("failed to write key file: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| {
            CrateError::KeyStorage(format!("failed to set key file permissions: {e}"))
        })?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// iOS: Keychain provider.
// ---------------------------------------------------------------------------

#[cfg(target_os = "ios")]
struct KeychainKeyProvider;

#[cfg(target_os = "ios")]
impl KeychainKeyProvider {
    fn new() -> Self {
        Self
    }
}

/// Keychain coordinates for the SQLCipher key. The service/account pair is stable across
/// launches; the item lands in this app's default access group, so no `keychain-access-groups`
/// entitlement is required.
#[cfg(target_os = "ios")]
const KEYCHAIN_SERVICE: &str = "audio.bbx.crate.sqlcipher";
#[cfg(target_os = "ios")]
const KEYCHAIN_ACCOUNT: &str = "db_key";

/// `errSecInteractionNotAllowed` — the item exists but cannot be read because the device has
/// not been unlocked since boot. `security-framework-sys` does not re-export this constant;
/// `-25308` is the stable Security.framework value.
#[cfg(target_os = "ios")]
const ERR_SEC_INTERACTION_NOT_ALLOWED: i32 = -25308;

#[cfg(target_os = "ios")]
impl KeyProvider for KeychainKeyProvider {
    fn get_or_create_key(&self) -> Result<String> {
        use security_framework::passwords::get_generic_password;
        use security_framework_sys::base::errSecItemNotFound;

        match get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
            Ok(bytes) => String::from_utf8(bytes).map_err(|e| {
                CrateError::KeyStorage(format!("keychain key is not valid UTF-8: {e}"))
            }),
            // No key yet (fresh install): generate one and store it encrypted-at-rest.
            Err(e) if e.code() == errSecItemNotFound => {
                let key = generate_key();
                store_key(&key)?;
                Ok(key)
            }
            // Device locked since boot — surface a distinct error. Crucially, do NOT treat
            // this as "no key" and generate a second one, which would mismatch an
            // already-encrypted database.
            Err(e) if e.code() == ERR_SEC_INTERACTION_NOT_ALLOWED => Err(CrateError::KeyStorage(
                "keychain is locked (device not unlocked since boot)".to_string(),
            )),
            Err(e) => Err(CrateError::KeyStorage(format!(
                "keychain read failed (OSStatus {}): {e}",
                e.code()
            ))),
        }
    }
}

/// Store the key in the Keychain with device-only, after-first-unlock accessibility.
#[cfg(target_os = "ios")]
fn store_key(key: &str) -> Result<()> {
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::passwords::{set_generic_password_options, PasswordOptions};

    let access_control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleAfterFirstUnlockThisDeviceOnly),
        0, // CFOptionFlags: accessibility only, no biometry/passcode constraint.
    )
    .map_err(|e| {
        CrateError::KeyStorage(format!(
            "failed to create keychain access control (OSStatus {}): {e}",
            e.code()
        ))
    })?;

    let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
    options.set_access_control(access_control);

    set_generic_password_options(key.as_bytes(), options).map_err(|e| {
        CrateError::KeyStorage(format!(
            "keychain write failed (OSStatus {}): {e}",
            e.code()
        ))
    })
}

// ---------------------------------------------------------------------------
// Android: Keystore provider (#144) — JNI bridge to CrateDbKey.kt.
// ---------------------------------------------------------------------------

/// Fully-qualified JNI class name of the Kotlin helper (see `gen/android/.../CrateDbKey.kt`).
#[cfg(target_os = "android")]
const DB_KEY_CLASS: &str = "com/bbx_audio/crateapp/CrateDbKey";

#[cfg(target_os = "android")]
struct AndroidKeystoreKeyProvider;

#[cfg(target_os = "android")]
impl AndroidKeystoreKeyProvider {
    fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "android")]
impl KeyProvider for AndroidKeystoreKeyProvider {
    /// Call `CrateDbKey.getOrCreateKey(context)` over JNI. The Kotlin side owns generation,
    /// Keystore wrapping, and the never-regenerate-over-an-existing-blob policy; a thrown
    /// exception (locked Keystore, corrupt blob, …) surfaces here as a `KeyStorage` error
    /// rather than a silent second key that would mismatch the encrypted database.
    fn get_or_create_key(&self) -> Result<String> {
        use jni::objects::{JString, JValue};

        crate::android_context::with_env_and_context(
            |env, context| {
                // RECONCILE: signature `(Landroid/content/Context;)Ljava/lang/String;`.
                let result = env
                    .call_static_method(
                        DB_KEY_CLASS,
                        "getOrCreateKey",
                        "(Landroid/content/Context;)Ljava/lang/String;",
                        &[JValue::Object(context)],
                    )
                    .map_err(|e| {
                        CrateError::KeyStorage(format!("CrateDbKey.getOrCreateKey failed: {e}"))
                    })?;

                let key_obj = result.l().map_err(|e| {
                    CrateError::KeyStorage(format!("android keystore key object: {e}"))
                })?;
                let key: String = env
                    .get_string(&JString::from(key_obj))
                    .map_err(|e| {
                        CrateError::KeyStorage(format!("android keystore key decode: {e}"))
                    })?
                    .into();
                if key.is_empty() {
                    return Err(CrateError::KeyStorage(
                        "android keystore returned an empty key".to_string(),
                    ));
                }
                Ok(key)
            },
            |m| CrateError::KeyStorage(format!("android keystore: {m}")),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests — the file provider is the only one exercisable on a desktop/CI host.
// ---------------------------------------------------------------------------

#[cfg(all(test, not(any(target_os = "ios", target_os = "android"))))]
mod tests {
    use super::*;

    fn unique_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "crate-key-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_provider_generates_then_reuses_key() {
        let dir = unique_temp_dir();
        let provider = FileKeyProvider::new(dir.clone());

        // First call generates + persists a 64-char key and writes db.key.
        let key1 = provider.get_or_create_key().unwrap();
        assert_eq!(key1.len(), 64);
        assert!(dir.join("db.key").exists());

        // A second call returns the same persisted key (no regeneration).
        let key2 = provider.get_or_create_key().unwrap();
        assert_eq!(key1, key2);

        std::fs::remove_dir_all(&dir).ok();
    }
}
