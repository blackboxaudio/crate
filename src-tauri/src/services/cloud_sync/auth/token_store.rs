//! Refresh-token storage in the encrypted SQLCipher database.
//!
//! The Firebase refresh token is stored in the `sync_state` key/value table.
//! Since the database is already SQLCipher-encrypted, no additional encryption
//! layer is needed.

use rusqlite::Connection;

use crate::error::Result;

const KEY: &str = "cloud_refresh_token";

pub fn store_refresh_token(conn: &Connection, token: &str) -> Result<()> {
    super::write_state(conn, KEY, token)
}

pub fn load_refresh_token(conn: &Connection) -> Result<Option<String>> {
    Ok(super::read_state(conn, KEY)?.filter(|s| !s.is_empty()))
}

pub fn clear_refresh_token(conn: &Connection) -> Result<()> {
    super::write_state(conn, KEY, "")
}
