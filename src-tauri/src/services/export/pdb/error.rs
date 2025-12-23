//! PDB-specific error types

#![allow(dead_code)]

use thiserror::Error;

use crate::error::CrateError;

/// Errors that can occur during PDB file operations
#[derive(Error, Debug)]
pub enum PdbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid page type: {0}")]
    InvalidPageType(u32),

    #[error("Invalid table type: {0}")]
    InvalidTableType(u32),

    #[error("Page overflow: row size {row_size} exceeds available space {available}")]
    PageOverflow { row_size: usize, available: usize },

    #[error("Binary read/write error: {0}")]
    BinRw(String),
}

impl From<binrw::Error> for PdbError {
    fn from(e: binrw::Error) -> Self {
        PdbError::BinRw(e.to_string())
    }
}

impl From<PdbError> for CrateError {
    fn from(e: PdbError) -> Self {
        CrateError::Export(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PdbError>;
