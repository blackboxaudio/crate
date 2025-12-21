use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use sysinfo::System;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::models::{DiagnosticEntry, DiagnosticLevel, DiagnosticsReport, SystemInfo};

const MAX_ENTRIES: usize = 100;

pub struct DiagnosticsService {
    entries: Arc<Mutex<VecDeque<DiagnosticEntry>>>,
    app_data_dir: PathBuf,
}

impl DiagnosticsService {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_ENTRIES))),
            app_data_dir,
        }
    }

    /// Log a diagnostic entry
    pub fn log(
        &self,
        level: DiagnosticLevel,
        category: &str,
        message: &str,
        details: Option<&str>,
    ) {
        let entry = DiagnosticEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            level,
            category: category.to_string(),
            message: message.to_string(),
            details: details.map(|s| s.to_string()),
        };

        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= MAX_ENTRIES {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    /// Log an error
    #[allow(dead_code)]
    pub fn log_error(&self, category: &str, message: &str, details: Option<&str>) {
        self.log(DiagnosticLevel::Error, category, message, details);
    }

    /// Log a warning
    #[allow(dead_code)]
    pub fn log_warning(&self, category: &str, message: &str, details: Option<&str>) {
        self.log(DiagnosticLevel::Warning, category, message, details);
    }

    /// Get all diagnostic entries
    pub fn get_entries(&self) -> Vec<DiagnosticEntry> {
        self.entries
            .lock()
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear all entries
    pub fn clear_entries(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    /// Get system information
    pub fn get_system_info(&self) -> SystemInfo {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let data_dir_size = self.calculate_dir_size(&self.app_data_dir);
        let db_size = std::fs::metadata(self.app_data_dir.join("crate.db"))
            .map(|m| m.len())
            .ok();

        SystemInfo {
            os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
            os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
            cpu_brand,
            cpu_cores: sys.cpus().len(),
            total_memory_bytes: sys.total_memory(),
            used_memory_bytes: sys.used_memory(),
            data_dir_size_bytes: data_dir_size,
            database_size_bytes: db_size,
        }
    }

    /// Generate a full diagnostics report
    pub fn generate_report(&self) -> DiagnosticsReport {
        DiagnosticsReport {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: option_env!("CRATE_ENV")
                .unwrap_or("development")
                .to_string(),
            generated_at: Utc::now(),
            system_info: self.get_system_info(),
            entries: self.get_entries(),
        }
    }

    fn calculate_dir_size(&self, path: &PathBuf) -> Option<u64> {
        if !path.exists() {
            return None;
        }

        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
            .reduce(|a, b| a + b)
    }
}
