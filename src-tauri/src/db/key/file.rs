//! Desktop key provider: a plaintext `db.key` file in the app data dir, locked
//! down to `0o600` on Unix.
//!
//! This is the pre-#134 behavior, unchanged — only moved behind the
//! [`KeyProvider`] abstraction. A sandboxed key file is the wrong primitive on
//! mobile (see the iOS/Android providers), but on desktop it remains the
//! established approach.

use std::path::Path;

use super::{generate_key, KeyProvider};
use crate::error::{CrateError, Result};

/// Stores the SQLCipher key as a `db.key` file alongside the database.
pub struct FileKeyProvider;

impl KeyProvider for FileKeyProvider {
    fn get_or_create(&self, app_data_dir: &Path) -> Result<String> {
        let key_path = app_data_dir.join("db.key");

        if key_path.exists() {
            let key = std::fs::read_to_string(&key_path)
                .map_err(|e| CrateError::KeyStorage(format!("failed to read key file: {e}")))?;
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Ok(key);
            }
        }

        // First launch: generate a random 64-char hex key.
        let key = generate_key();
        write_key_file(&key_path, &key)?;
        Ok(key)
    }
}

/// Write the key to a file with restrictive permissions.
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
