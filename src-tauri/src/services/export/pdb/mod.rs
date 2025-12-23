//! PDB file format module for Rekordbox export
//!
//! This module provides functionality to write PDB (export.pdb) and extended PDB
//! (exportExt.pdb) files that are compatible with Pioneer CDJ/XDJ equipment.
//!
//! # Structure
//!
//! PDB files use a page-based format with 4096-byte pages:
//! - Page 0: File header with table descriptors
//! - Each table has an index page followed by data pages
//! - Data is stored in a heap with row groups indexing the rows
//!
//! # Usage
//!
//! ```ignore
//! use crate::services::export::pdb::PdbWriter;
//!
//! let mut writer = PdbWriter::new();
//! let track_id = writer.add_track(&track, "path/to/track.mp3", "/PIONEER/USBANLZ/...");
//! writer.add_playlist(&playlist, &[track_id]);
//! writer.write(Path::new("/Volumes/USB/PIONEER/rekordbox/export.pdb"))?;
//! ```

pub mod constants;
pub mod error;
pub mod ext_writer;
pub mod header;
pub mod index;
pub mod merger;
pub mod page;
pub mod reader;
pub mod strings;
pub mod tables;
pub mod types;
pub mod writer;

// Re-exports for convenience
pub use writer::RekordboxPdbWriter;
