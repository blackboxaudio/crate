//! Follow artists & labels (issue #126).
//!
//! A `FollowedSource` is an artist or label page Crate watches for new releases. The
//! synced row lives in `followed_sources`; per-device watch state lives in the local
//! `followed_source_state` (last-checked, health, baseline flag) and
//! `followed_source_releases` (every URL seen under the source + its disposition).
//!
//! `crud` holds the synchronous DB methods; `watch` holds the per-source check logic
//! (the forward-looking baseline + incremental surfacing) reused by both the manual
//! "Check now" commands and the background timer; `diff` holds pure, tested helpers.

mod crud;
pub mod diff;
pub mod watch;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// A followed source reduced to what the watch loop needs to check it.
pub struct SourceToCheck {
    pub id: String,
    pub url: String,
    pub source_type: String,
    pub follow_type: String,
    pub name: Option<String>,
    pub baseline_established: bool,
}

pub struct FollowService {
    conn: Arc<Mutex<Connection>>,
    app_data_dir: PathBuf,
}

impl FollowService {
    pub fn new(conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) -> Self {
        Self { conn, app_data_dir }
    }

    /// Clone of the DB connection Arc for use in background tasks.
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub fn app_data_dir(&self) -> PathBuf {
        self.app_data_dir.clone()
    }
}
