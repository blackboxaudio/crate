use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde_json;

use crate::error::{CrateError, Result};
use crate::models::AppSettings;
use crate::services::cloud_sync::{self, pipeline::dirty};

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

        let date_format = self
            .get_setting_value(&conn, "date_format")?
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

        let continuous_playback = self
            .get_setting_value(&conn, "continuous_playback")?
            .map(|v| v == "true")
            .unwrap_or(true);

        let auto_fetch_metadata = self
            .get_setting_value(&conn, "auto_fetch_metadata")?
            .map(|v| v != "false")
            .unwrap_or(true);

        let transfer_tags_on_import = self
            .get_setting_value(&conn, "transfer_tags_on_import")?
            .map(|v| v != "false")
            .unwrap_or(true);

        let remove_release_after_import = self
            .get_setting_value(&conn, "remove_release_after_import")?
            .map(|v| v != "false")
            .unwrap_or(true);

        // Parse ignored device IDs from JSON array, default to empty
        let ignored_device_ids = self
            .get_setting_value(&conn, "ignored_device_ids")?
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();

        let last_backup_at = self.get_setting_value(&conn, "last_backup_at")?;

        let backup_frequency = self
            .get_setting_value(&conn, "backup_frequency")?
            .and_then(|v| v.parse().ok())
            .unwrap_or_default();

        let last_backup_type = self.get_setting_value(&conn, "last_backup_type")?;

        let has_completed_onboarding = self
            .get_setting_value(&conn, "has_completed_onboarding")?
            .map(|v| v == "true")
            .unwrap_or_else(|| {
                // No explicit onboarding key — check if user has any library tracks.
                // If yes, this is an existing user upgrading → skip onboarding.
                // We check tracks rather than settings rows to avoid false positives
                // from auto-backup writing settings on a fresh install.
                conn.query_row("SELECT COUNT(*) FROM tracks LIMIT 1", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap_or(0)
                    > 0
            });

        let has_completed_wizard = self
            .get_setting_value(&conn, "has_completed_wizard")?
            .map(|v| v == "true")
            .unwrap_or_else(|| {
                // Existing users upgrading (have tracks) → skip auto-tour
                conn.query_row("SELECT COUNT(*) FROM tracks LIMIT 1", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap_or(0)
                    > 0
            });

        Ok(AppSettings {
            theme,
            accent_color,
            font,
            audio_device,
            language,
            key_notation_format,
            date_format,
            auto_analyze_on_import,
            auto_sync_on_connect,
            auto_sync_on_change,
            continuous_playback,
            auto_fetch_metadata,
            transfer_tags_on_import,
            remove_release_after_import,
            ignored_device_ids,
            last_backup_at,
            backup_frequency,
            last_backup_type,
            has_completed_onboarding,
            has_completed_wizard,
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

        // Only whitelisted settings sync; device-local keys never leave this device.
        // Per-key HLCs live in `sync_state` so the `settings` table shape is untouched.
        // Best-effort: a sync-bookkeeping hiccup never blocks a settings change.
        if cloud_sync::is_synced_setting(key) {
            dirty::stamp_setting(&conn, key);
        }

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
