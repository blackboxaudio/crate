//! ANLZ-specific error types

#![allow(dead_code)]

use thiserror::Error;

use crate::error::CrateError;

/// Errors that can occur during ANLZ file operations
#[derive(Error, Debug)]
pub enum AnlzError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid magic bytes: expected {expected}, got {actual}")]
    InvalidMagic { expected: String, actual: String },

    #[error("Invalid tag: {0}")]
    InvalidTag(String),

    #[error("Invalid variant: {0}")]
    InvalidVariant(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Binary read/write error: {0}")]
    BinRw(String),
}

impl From<binrw::Error> for AnlzError {
    fn from(e: binrw::Error) -> Self {
        AnlzError::BinRw(e.to_string())
    }
}

impl From<AnlzError> for CrateError {
    fn from(e: AnlzError) -> Self {
        CrateError::Export(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AnlzError>;
