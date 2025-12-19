use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::AppSettings;

pub struct SettingsService {
    conn: Arc<Mutex<Connection>>,
}

impl SettingsService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let theme = self
            .get_setting_value(&conn, "theme")?
            .and_then(|v| v.parse().ok())
            .unwrap_or_default();

        let accent_color = self
            .get_setting_value(&conn, "accent_color")?
            .and_then(|v| v.parse().ok())
            .unwrap_or_default();

        Ok(AppSettings {
            theme,
            accent_color,
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;

        Ok(())
    }

    fn get_setting_value(&self, conn: &Connection, key: &str) -> Result<Option<String>> {
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }
}
