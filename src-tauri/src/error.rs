use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrateError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Import error: {0}")]
    Import(String),

    #[allow(dead_code)]
    #[error("Export error: {0}")]
    Export(String),

    #[error("Device error: {0}")]
    Device(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Metadata error: {0}")]
    Metadata(String),

    #[error("Artwork error: {0}")]
    Artwork(String),
}

impl serde::Serialize for CrateError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CrateError>;
