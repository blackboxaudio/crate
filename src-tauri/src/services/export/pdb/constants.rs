//! PDB format constants
//!
//! Based on the Deep Symmetry analysis:
//! https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html

/// Page size for PDB files (standard is 4096 bytes)
pub const PAGE_SIZE: u32 = 4096;

/// Size of the file header (before table descriptors)
pub const FILE_HEADER_SIZE: usize = 0x1C; // 28 bytes

/// Size of a table descriptor in the file header
pub const TABLE_DESCRIPTOR_SIZE: usize = 16;

/// Offset where heap data begins in a data page
pub const HEAP_START_OFFSET: usize = 0x28; // 40 bytes

/// Page flags for index pages
pub const PAGE_FLAGS_INDEX: u8 = 0x64;

/// Page flags for data pages
pub const PAGE_FLAGS_DATA: u8 = 0x24;

/// Null page marker (indicates no next page)
pub const NULL_PAGE_MARKER: u32 = 0x03FF_FFFF;

/// Empty index entry marker
pub const EMPTY_INDEX_ENTRY: u32 = 0x1FFF_FFF8;

/// Maximum rows per row group
pub const MAX_ROWS_PER_GROUP: usize = 16;

/// Index page magic value
pub const INDEX_MAGIC: u16 = 0x03EC;

/// Empty marker used in index headers
pub const INDEX_EMPTY_MARKER: u16 = 0x1FFF;

/// Second magic value for index pages
pub const INDEX_MAGIC2: u64 = 0x0000_0000_03FF_FFFF;

/// Maximum length for short ASCII DeviceSQL strings
pub const MAX_SHORT_STRING_LEN: usize = 126;
