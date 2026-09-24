//! Read/write-split database handle.
//!
//! One process-wide WRITER connection (the same `Arc<Mutex<Connection>>` every legacy
//! service holds) plus a small pool of read-only reader connections. Under WAL, readers
//! get snapshot isolation and never block on — or are blocked by — the writer, so hot
//! read paths (preview stream lookups, the proxy's cache checks, feed pagination) stay
//! responsive while a cloud-sync merge or cache write holds the writer.
//!
//! Correctness rules:
//! - All writes stay on the single writer (`Db::write` / the legacy writer Arc) — the
//!   single-writer invariant is preserved by construction.
//! - Never run cloud-sync bookkeeping (`pipeline::dirty::*`, HLC ticks) inside a `read`
//!   closure: those are read-modify-write sequences that rely on the writer mutex as
//!   their critical section.
//! - A `read` closure runs inside one deferred read transaction, so multi-statement
//!   reads see a single consistent snapshot.

use rusqlite::Connection;
use std::sync::{Arc, Condvar, Mutex};

use crate::error::{CrateError, Result};

#[derive(Clone)]
pub struct Db {
    /// The same Arc legacy services hold — all writes and unmigrated reads.
    writer: Arc<Mutex<Connection>>,
    /// Read-only pool. `None` = fallback mode (tests, WAL unavailable): reads go
    /// through the writer mutex, which is exactly the pre-split behavior.
    readers: Option<Arc<ReaderPool>>,
}

impl Db {
    pub fn new(writer: Arc<Mutex<Connection>>, readers: Option<Arc<ReaderPool>>) -> Self {
        Self { writer, readers }
    }

    /// A handle with no reader pool: reads fall back to the writer mutex. For tests
    /// (in-memory DBs can't share) and for constructors that only have the legacy Arc.
    pub fn from_writer(writer: Arc<Mutex<Connection>>) -> Self {
        Self {
            writer,
            readers: None,
        }
    }

    /// Snapshot-isolated read on a pooled reader connection (or the writer when no
    /// pool exists). The closure runs inside one deferred read transaction.
    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        match &self.readers {
            Some(pool) => pool.with_conn(|conn| {
                let tx = conn.unchecked_transaction()?;
                // Transaction derefs to Connection; drop = rollback (a no-op for reads).
                f(&tx)
            }),
            None => self.write(f),
        }
    }

    /// Serialized access to the single writer connection (unchanged semantics vs the
    /// legacy `conn.lock()`).
    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.writer.lock().map_err(|_| CrateError::LockPoisoned)?;
        f(&guard)
    }

    /// The legacy writer Arc, for services/paths not yet migrated to this handle.
    pub fn writer(&self) -> Arc<Mutex<Connection>> {
        self.writer.clone()
    }
}

pub struct ReaderPool {
    conns: Mutex<Vec<Connection>>,
    available: Condvar,
}

impl ReaderPool {
    pub fn new(conns: Vec<Connection>) -> Self {
        Self {
            conns: Mutex::new(conns),
            available: Condvar::new(),
        }
    }

    /// Run `f` on a pooled connection, blocking briefly if all readers are busy. The
    /// connection is returned to the pool via RAII, so a panicking closure can't leak it.
    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = {
            let mut guard = self.conns.lock().map_err(|_| CrateError::LockPoisoned)?;
            loop {
                if let Some(conn) = guard.pop() {
                    break conn;
                }
                guard = self
                    .available
                    .wait(guard)
                    .map_err(|_| CrateError::LockPoisoned)?;
            }
        };
        let returner = ReaderReturn {
            pool: self,
            conn: Some(conn),
        };
        f(returner.conn.as_ref().expect("reader present"))
    }
}

/// Returns the pooled connection on drop (including on panic/unwind).
struct ReaderReturn<'a> {
    pool: &'a ReaderPool,
    conn: Option<Connection>,
}

impl Drop for ReaderReturn<'_> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            if let Ok(mut guard) = self.pool.conns.lock() {
                guard.push(conn);
                self.pool.available.notify_one();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// WAL needs a real file (in-memory DBs can't share), so these tests use a temp dir.
    fn temp_db(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("crate-db-handle-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.db")
    }

    fn open_pair(path: &PathBuf) -> (Connection, Connection) {
        let writer = Connection::open(path).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        writer.pragma_update(None, "busy_timeout", 5000).unwrap();
        writer
            .execute_batch("CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT);")
            .unwrap();
        let reader = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        reader.pragma_update(None, "busy_timeout", 5000).unwrap();
        (writer, reader)
    }

    #[test]
    fn read_completes_while_writer_transaction_open() {
        let path = temp_db("concurrent");
        let (writer, reader) = open_pair(&path);
        writer
            .execute("INSERT INTO t (v) VALUES ('committed')", [])
            .unwrap();

        let db = Db::new(
            Arc::new(Mutex::new(writer)),
            Some(Arc::new(ReaderPool::new(vec![reader]))),
        );

        // Open a writer transaction with uncommitted rows and HOLD the mutex.
        let writer_arc = db.writer();
        let guard = writer_arc.lock().unwrap();
        guard.execute_batch("BEGIN IMMEDIATE;").unwrap();
        for i in 0..500 {
            guard
                .execute(
                    "INSERT INTO t (v) VALUES (?1)",
                    [format!("uncommitted-{i}")],
                )
                .unwrap();
        }

        // The read must not block on the held mutex, and snapshot isolation must hide
        // the uncommitted rows.
        let count: i64 = db
            .read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1, "reader must not see uncommitted writer rows");

        guard.execute_batch("COMMIT;").unwrap();
        drop(guard);

        let count: i64 = db
            .read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 501, "reader sees committed rows in a fresh snapshot");
    }

    #[test]
    fn fallback_without_pool_uses_writer() {
        let path = temp_db("fallback");
        let (writer, _reader) = open_pair(&path);
        writer
            .execute("INSERT INTO t (v) VALUES ('x')", [])
            .unwrap();
        let db = Db::from_writer(Arc::new(Mutex::new(writer)));
        let count: i64 = db
            .read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn pool_returns_connection_after_use() {
        let path = temp_db("pool");
        let (writer, reader) = open_pair(&path);
        let db = Db::new(
            Arc::new(Mutex::new(writer)),
            Some(Arc::new(ReaderPool::new(vec![reader]))),
        );
        // A single-reader pool would deadlock on the second read if the first leaked.
        for _ in 0..3 {
            db.read(|conn| Ok(conn.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?))
                .unwrap();
        }
    }
}
