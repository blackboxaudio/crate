pub mod handle;
mod key_provider;
pub mod schema;

use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::{CrateError, Result};

pub use handle::Db;
use handle::ReaderPool;

/// Number of read-only pooled connections. Small on purpose: WAL readers never block
/// each other or the writer, so this only needs to cover concurrent hot paths (the
/// stream proxy + a feed query + one spare). Each open pays the SQLCipher KDF
/// (~50-300 ms), so they open once at startup, in parallel.
const READER_POOL_SIZE: usize = 3;

pub struct Database {
    conn: Arc<Mutex<Connection>>,
    readers: Option<Arc<ReaderPool>>,
}

/// Check whether a database file is unencrypted by attempting to read its header.
/// An unencrypted SQLite database starts with "SQLite format 3\0".
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn is_unencrypted(db_path: &std::path::Path) -> bool {
    std::fs::read(db_path)
        .map(|bytes| bytes.starts_with(b"SQLite format 3\0"))
        .unwrap_or(false)
}

/// Migrate an existing unencrypted database to an encrypted one using `sqlcipher_export`.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn migrate_to_encrypted(db_path: &std::path::Path, key: &str) -> Result<()> {
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

/// Migrate a pre-existing unencrypted database to encrypted form, if needed (desktop only).
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn migrate_if_unencrypted(db_path: &std::path::Path, key: &str) -> Result<()> {
    if db_path.exists() && is_unencrypted(db_path) {
        log::info!("Migrating unencrypted database to encrypted format");
        migrate_to_encrypted(db_path, key)?;
    }
    Ok(())
}

/// Mobile databases are encrypted from creation, so there is never anything to migrate.
#[cfg(any(target_os = "ios", target_os = "android"))]
fn migrate_if_unencrypted(_db_path: &std::path::Path, _key: &str) -> Result<()> {
    Ok(())
}

/// Apply the per-connection pragma set. The SQLCipher `key` MUST be the first statement
/// on every connection. The writer additionally sets `journal_mode=WAL` (persistent in
/// the DB file, verified — SQLCipher encrypts WAL frames) and `synchronous=NORMAL`
/// (safe under WAL, skips the per-commit main-file fsync). Returns whether WAL is
/// active; when it isn't (exotic filesystem), the caller skips the reader pool and
/// everything degrades to the single-connection behavior.
///
/// NOTE: WAL means `crate.db` alone is not the whole database on disk (`-wal`/`-shm`
/// live next to it). Any future feature that file-copies the DB must run
/// `PRAGMA wal_checkpoint(TRUNCATE)` first (see the exit handler in `lib.rs`) or copy
/// all three files. Current backup (row-level JSON), cloud sync (bucket blobs), and
/// device export (separate DB file) are unaffected.
fn configure_connection(conn: &Connection, key: &str, writer: bool) -> Result<bool> {
    conn.pragma_update(None, "key", key)?;
    let mut wal_active = true;
    if writer {
        let mode: String =
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
        wal_active = mode.eq_ignore_ascii_case("wal");
        if wal_active {
            conn.pragma_update(None, "synchronous", "NORMAL")?;
        } else {
            log::warn!("journal_mode=WAL not applied (got {mode}); reader pool disabled");
        }
    }
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    Ok(wal_active)
}

/// Open the read-only reader pool in parallel threads (each open pays the SQLCipher
/// KDF). Best-effort: any failure disables the pool rather than failing startup.
fn open_reader_pool(db_path: &std::path::Path, key: &str) -> Option<Arc<ReaderPool>> {
    let handles: Vec<_> = (0..READER_POOL_SIZE)
        .map(|_| {
            let path = db_path.to_path_buf();
            let key = key.to_string();
            std::thread::spawn(move || -> Result<Connection> {
                let conn = Connection::open_with_flags(
                    &path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                configure_connection(&conn, &key, false)?;
                Ok(conn)
            })
        })
        .collect();
    let mut conns = Vec::with_capacity(READER_POOL_SIZE);
    for h in handles {
        match h.join() {
            Ok(Ok(conn)) => conns.push(conn),
            Ok(Err(e)) => {
                log::warn!("db: reader connection open failed ({e}); reader pool disabled");
                return None;
            }
            Err(_) => {
                log::warn!("db: reader connection open panicked; reader pool disabled");
                return None;
            }
        }
    }
    Some(Arc::new(ReaderPool::new(conns)))
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        Self::new_inner(db_path, true)
    }

    /// Writer-only open (no reader pool): for short-lived headless contexts (the Android
    /// WorkManager sync) where pooled readers would never be used but each would still
    /// pay the SQLCipher KDF.
    pub fn new_writer_only(db_path: PathBuf) -> Result<Self> {
        Self::new_inner(db_path, false)
    }

    fn new_inner(db_path: PathBuf, open_readers: bool) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let app_data_dir = db_path.parent().ok_or_else(|| {
            CrateError::KeyStorage("database path has no parent directory".to_string())
        })?;
        let key = key_provider::for_platform(app_data_dir).get_or_create_key()?;

        // If an existing database is unencrypted, migrate it (desktop only). Mobile
        // databases are encrypted from creation, so this is a no-op there.
        migrate_if_unencrypted(&db_path, &key)?;

        let conn = Connection::open(&db_path)?;
        let wal_active = configure_connection(&conn, &key, true)?;
        log::info!(
            "db: opened writer (journal_mode={})",
            if wal_active { "wal" } else { "legacy" }
        );

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            readers: None,
        };

        // Run migrations on the writer BEFORE opening readers (a read-only connection
        // can't create the schema, and WAL's `-shm` needs a live writer first).
        db.migrate()?;

        let readers = if wal_active && open_readers {
            open_reader_pool(&db_path, &key)
        } else {
            None
        };
        Ok(Self { readers, ..db })
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    /// The read/write-split handle: pooled snapshot reads + the shared writer.
    pub fn handle(&self) -> Db {
        Db::new(self.conn.clone(), self.readers.clone())
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        run_migrations(&conn)
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            readers: self.readers.clone(),
        }
    }
}

/// Apply any pending schema migrations to `conn`, version-gated and atomic.
///
/// Each migration's DDL and its `schema_version` bump commit together in one
/// transaction, so an interrupted run (e.g. the process is killed mid-migration)
/// rolls back cleanly and is retried from scratch on the next launch — never
/// leaving a half-applied schema. Migrations run in order, each exactly once.
fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY)",
        [],
    )?;

    let current_version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (idx, sql) in schema::get_migrations().iter().enumerate() {
        let version = idx as i32 + 1;
        if version > current_version {
            log::info!("Running migration {version}");
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [version],
            )?;
            tx.commit()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::OptionalExtension;

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |_| Ok(()),
        )
        .optional()
        .unwrap()
        .is_some()
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        cols.iter().any(|c| c == column)
    }

    fn version(conn: &Connection) -> i32 {
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Every sync table + rooting/`_hlc` column that migrations 3–4 must create.
    fn assert_sync_schema(conn: &Connection) {
        for t in [
            "library_roots",
            "sync_root_mappings",
            "sync_tombstones",
            "sync_dirty_buckets",
            "sync_state",
        ] {
            assert!(table_exists(conn, t), "expected table `{t}` to exist");
        }
        for (tbl, col) in [
            ("tracks", "_hlc"),
            ("tracks", "library_root_id"),
            ("tracks", "relative_path"),
            ("playlists", "_hlc"),
            ("playlist_tracks", "_hlc"),
            ("cues", "_hlc"),
            ("tag_categories", "_hlc"),
            ("tags", "_hlc"),
            ("track_tags", "_hlc"),
            ("discovery_releases", "_hlc"),
            ("discovery_tracks", "_hlc"),
            ("discovery_release_tags", "_hlc"),
            ("playlist_discovery_releases", "_hlc"),
        ] {
            assert!(
                column_exists(conn, tbl, col),
                "expected column `{tbl}.{col}` to exist"
            );
        }
    }

    #[test]
    fn fresh_db_migrates_to_latest_and_reruns_cleanly() {
        let conn = open_mem();
        run_migrations(&conn).unwrap();

        assert_sync_schema(&conn);
        let latest = schema::get_migrations().len() as i32;
        assert_eq!(version(&conn), latest);

        // Re-running must be a version-gated no-op, never an error.
        run_migrations(&conn).unwrap();
        assert_eq!(version(&conn), latest);
    }

    #[test]
    fn existing_v2_database_upgrades_cleanly() {
        let conn = open_mem();

        // Simulate a shipped (schema v2) database: apply only migrations 1 & 2.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY)",
            [],
        )
        .unwrap();
        for (idx, sql) in schema::get_migrations().iter().take(2).enumerate() {
            conn.execute_batch(sql).unwrap();
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [(idx as i32) + 1],
            )
            .unwrap();
        }
        assert_eq!(version(&conn), 2);
        assert!(!table_exists(&conn, "sync_state"));

        // Upgrading applies only the new migrations (3 & 4), atomically.
        run_migrations(&conn).unwrap();
        assert_sync_schema(&conn);
        assert_eq!(version(&conn), schema::get_migrations().len() as i32);
    }
}
