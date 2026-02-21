pub mod schema;

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::{CrateError, Result};

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

/// Retrieve or generate the database encryption key from a local key file.
fn get_or_create_db_key(app_data_dir: &Path) -> Result<String> {
    let key_path = app_data_dir.join("db.key");

    if key_path.exists() {
        let key = std::fs::read_to_string(&key_path)
            .map_err(|e| CrateError::KeyStorage(format!("failed to read key file: {e}")))?;
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // First launch: generate a random 64-char hex key using two UUIDs
    let key = format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    );

    write_key_file(&key_path, &key)?;

    Ok(key)
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

/// Check whether a database file is unencrypted by attempting to read its header.
/// An unencrypted SQLite database starts with "SQLite format 3\0".
fn is_unencrypted(db_path: &Path) -> bool {
    std::fs::read(db_path)
        .map(|bytes| bytes.starts_with(b"SQLite format 3\0"))
        .unwrap_or(false)
}

/// Migrate an existing unencrypted database to an encrypted one using `sqlcipher_export`.
fn migrate_to_encrypted(db_path: &Path, key: &str) -> Result<()> {
    let conn = Connection::open(db_path)?;
    let encrypted_path = db_path.with_extension("db.encrypted");

    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS encrypted KEY '{}';
         SELECT sqlcipher_export('encrypted');
         DETACH DATABASE encrypted;",
        encrypted_path.display(),
        key
    ))?;

    drop(conn);
    std::fs::rename(&encrypted_path, db_path)?;
    Ok(())
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let app_data_dir = db_path.parent().ok_or_else(|| {
            CrateError::KeyStorage("database path has no parent directory".to_string())
        })?;
        let key = get_or_create_db_key(app_data_dir)?;

        // If the database file already exists and is unencrypted, migrate it
        if db_path.exists() && is_unencrypted(&db_path) {
            log::info!("Migrating unencrypted database to encrypted format");
            migrate_to_encrypted(&db_path, &key)?;
        }

        let conn = Connection::open(&db_path)?;

        // Apply encryption key
        conn.pragma_update(None, "key", &key)?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Run migrations
        db.migrate()?;

        Ok(db)
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    fn migrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // Create schema version table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )?;

        // Get current version
        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Run migrations
        let migrations = schema::get_migrations();

        for (version, sql) in migrations.iter().enumerate() {
            let version = version as i32 + 1;
            if version > current_version {
                log::info!("Running migration {version}");
                conn.execute_batch(sql)?;
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [version],
                )?;
            }
        }

        Ok(())
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}
