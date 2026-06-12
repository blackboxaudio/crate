//! Provisioning of the SQLCipher database encryption key.
//!
//! The key is supplied by a platform-specific [`KeyProvider`], chosen at compile
//! time by [`provision_key`]:
//!
//! - **Desktop** (`feature = "desktop"`): a plaintext `db.key` file with `0o600`
//!   permissions in the app data dir ([`file::FileKeyProvider`]) — unchanged from
//!   before this module existed.
//! - **iOS** (`target_os = "ios"`): the Keychain ([`ios::IosKeychainProvider`]),
//!   stored after-first-unlock and this-device-only.
//! - **Android** (`target_os = "android"`): the Keystore
//!   ([`android::AndroidKeystoreProvider`]).
//!
//! On mobile the key never touches the filesystem as plaintext, which is what
//! backs the "encrypted at rest" guarantee (#134).

use std::path::Path;

use crate::error::Result;

#[cfg(feature = "desktop")]
mod file;

#[cfg(all(not(feature = "desktop"), target_os = "ios"))]
mod ios;

#[cfg(all(not(feature = "desktop"), target_os = "android"))]
mod android;

/// Supplies the SQLCipher database encryption key for the current platform.
pub trait KeyProvider {
    /// Return the existing key, or generate and persist one on first launch.
    ///
    /// `app_data_dir` is the per-install data directory; the file provider keeps
    /// its key there, while the OS-backed providers ignore it (they key off a
    /// fixed service/account in the secure store).
    ///
    /// A transient backend failure MUST surface as an `Err` — never a freshly
    /// generated key — otherwise the existing encrypted database is orphaned and
    /// the user's library becomes unrecoverable.
    fn get_or_create(&self, app_data_dir: &Path) -> Result<String>;
}

/// Generate a fresh 64-char lowercase-hex key (two v4 UUIDs concatenated).
///
/// The format is identical on every platform so `PRAGMA key` behaves the same
/// everywhere. Centralized here so all providers stay in sync.
pub(crate) fn generate_key() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    )
}

/// Provision the database key using the platform-appropriate provider.
///
/// Exactly one arm is compiled per build (see the module docs). The final arm is
/// a compile-time guard: a non-desktop build for a platform without a secure
/// store fails to compile rather than silently shipping without key protection.
pub fn provision_key(app_data_dir: &Path) -> Result<String> {
    #[cfg(feature = "desktop")]
    let provider = file::FileKeyProvider;

    #[cfg(all(not(feature = "desktop"), target_os = "ios"))]
    let provider = ios::IosKeychainProvider;

    #[cfg(all(not(feature = "desktop"), target_os = "android"))]
    let provider = android::AndroidKeystoreProvider;

    // A non-desktop build for some other target has no secure store wired up: fail at
    // compile time rather than silently shipping without key protection. (`provider` is
    // also left unbound here, so this configuration cannot compile by accident.)
    #[cfg(all(
        not(feature = "desktop"),
        not(target_os = "ios"),
        not(target_os = "android")
    ))]
    compile_error!(
        "No KeyProvider for this target: a non-desktop build must target iOS or Android. \
         Enable the `desktop` feature, or add a provider in src/db/key/."
    );

    provider.get_or_create(app_data_dir)
}
