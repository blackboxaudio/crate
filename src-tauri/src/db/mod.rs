pub mod schema;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::{CrateError, Result};

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

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
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

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
                log::info!("Running migration {}", version);
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
