//! Device Library Plus export module.
//!
//! This module provides support for exporting to the Device Library Plus format,
//! an SQLCipher-encrypted SQLite database used by newer Pioneer DJ hardware:
//! - OPUS-QUAD
//! - OMNIS-DUO
//! - XDJ-AZ
//!
//! The format is an alternative to the legacy PDB (Rekordbox export) format and
//! offers a more modern SQLite-based structure with full relational integrity.

mod database;
mod encryption;
mod models;
mod schema;

pub use database::DeviceLibraryPlusWriter;
pub use models::*;
