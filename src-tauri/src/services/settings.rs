use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde_json;

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

        let font = self
            .get_setting_value(&conn, "font")?
            .and_then(|v| v.parse().ok())
            .unwrap_or_default();

        let audio_device = self.get_setting_value(&conn, "audio_device")?;

        let language = self
            .get_setting_value(&conn, "language")?
            .and_then(|v| v.parse().ok())
            .unwrap_or_default();

        let key_notation_format = self
            .get_setting_value(&conn, "key_notation_format")?
            .and_then(|v| v.parse().ok())
            .unwrap_or_default();

        // Default to true if not set (enabled by default)
        let auto_analyze_on_import = self
            .get_setting_value(&conn, "auto_analyze_on_import")?
            .map(|v| v != "false")
            .unwrap_or(true);

        // Default to false if not set (disabled by default)
        let auto_sync_on_connect = self
            .get_setting_value(&conn, "auto_sync_on_connect")?
            .map(|v| v == "true")
            .unwrap_or(false);

        let auto_sync_on_change = self
            .get_setting_value(&conn, "auto_sync_on_change")?
            .map(|v| v == "true")
            .unwrap_or(false);

        // Parse ignored device IDs from JSON array, default to empty
        let ignored_device_ids = self
            .get_setting_value(&conn, "ignored_device_ids")?
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();

        Ok(AppSettings {
            theme,
            accent_color,
            font,
            audio_device,
            language,
            key_notation_format,
            auto_analyze_on_import,
            auto_sync_on_connect,
            auto_sync_on_change,
            ignored_device_ids,
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
        let result = conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        });

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }
}
