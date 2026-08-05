//! Purchased-collection accounts (Bandcamp fan collections).
//!
//! A `CollectionAccount` is a linked fan-page URL whose public purchase collection
//! Crate scrapes. The synced rows live in `collection_accounts` / `collection_items`;
//! per-device refresh state lives in the local `collection_account_state`. Ownership of
//! discovery releases/tracks is derived from item URLs at read time, never stored.
//!
//! `crud` holds the synchronous DB methods; `scrape` holds the async fan-page
//! pagination that fills `collection_items`.

mod crud;
mod matching;
pub mod scrape;
pub mod watch;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// A linked account reduced to what the refresh loop needs to scrape it. (The scrape
/// always starts from the fan page itself, which returns `fan_id`, so the stored
/// `external_id` isn't carried.)
pub struct AccountToRefresh {
    pub id: String,
    pub url: String,
    pub name: Option<String>,
}

pub struct CollectionService {
    conn: Arc<Mutex<Connection>>,
    app_data_dir: PathBuf,
}

impl CollectionService {
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
