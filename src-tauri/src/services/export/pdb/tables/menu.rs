//! Menu and column row building for PDB files

use crate::services::export::pdb::strings::DeviceSQLString;

/// Standard column definitions for CDJ/XDJ menu
///
/// Format: (id, content_pointer, name)
pub const COLUMN_DEFS: &[(u16, u16, &str)] = &[
    (1, 128, "GENRE"),
    (2, 129, "ARTIST"),
    (3, 130, "ALBUM"),
    (4, 131, "TRACK"),
    (5, 132, "PLAYLIST"),
    (6, 133, "BPM"),
    (7, 134, "RATING"),
    (8, 135, "YEAR"),
    (9, 136, "REMIXER"),
    (10, 137, "LABEL"),
    (11, 138, "ORIGINAL ARTIST"),
    (12, 139, "KEY"),
    (13, 140, "DATE ADDED"),
    (14, 142, "COLOR"),
    (15, 143, "TIME"),
    (16, 144, "BITRATE"),
    (17, 145, "FILENAME"),
    (18, 146, "HISTORY"),
    (19, 141, "COMMENT"),
    (20, 147, "DJ PLAY COUNT"),
    (21, 148, "MY TAG"),
    (22, 149, "HOT CUE BANK"),
    (23, 150, "SEARCH"),
    (24, 151, "FOLDER"),
    (25, 152, "TRACK FILTER"),
    (26, 153, "MASTER TEMPO"),
    (27, 154, "QUANTIZE"),
];

/// Standard menu definitions for CDJ/XDJ root navigation
///
/// Based on rekordcrate test data from actual rekordbox exports.
/// Format: (category_id, content_pointer, unknown_type, visibility, sort_order)
/// - category_id: References Columns table ID for display label
/// - content_pointer: Points to data source (e.g., 2 = Artists table)
/// - unknown_type: 0x01=Track, 0x02=Artist, 0x03=Album, 0x05=BPM, 0x63=Generic
/// - visibility: 0=Visible, 1=Hidden
/// - sort_order: Position in menu (lower = higher, 0 for hidden)
pub const STANDARD_MENUS: &[(u16, u16, u8, u8, u16)] = &[
    // Visible menus (in sort_order)
    (2, 2, 0x02, 0, 1),   // ARTIST
    (3, 3, 0x03, 0, 2),   // ALBUM
    (4, 4, 0x01, 0, 3),   // TRACK
    (11, 12, 0x63, 0, 4), // KEY
    (17, 5, 0x63, 0, 5),  // PLAYLIST
    (19, 22, 0x63, 0, 6), // HISTORY
    (20, 18, 0x63, 0, 7), // SEARCH
    // Hidden menus
    (1, 1, 0x63, 1, 0),   // GENRE
    (5, 6, 0x05, 1, 0),   // BPM
    (6, 7, 0x63, 1, 0),   // RATING
    (7, 8, 0x63, 1, 0),   // YEAR
    (8, 9, 0x63, 1, 0),   // REMIXER
    (9, 10, 0x63, 1, 0),  // LABEL
    (10, 11, 0x63, 1, 0), // ORIGINAL ARTIST
    (13, 15, 0x63, 1, 0), // COLOR
    (14, 19, 0x04, 1, 0), // TIME
    (15, 20, 0x06, 1, 0), // BITRATE
    (16, 21, 0x63, 1, 0), // FILENAME
    (18, 23, 0x63, 1, 0), // COMMENTS
];

/// Build a column row (menu column definition)
///
/// Format:
/// - 0x00-0x01: ID
/// - 0x02-0x03: Content pointer
/// - 0x04+: Name string
pub fn build_column_row(id: u16, content_ptr: u16, name: &str) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: ID
    row.extend_from_slice(&id.to_le_bytes());
    // 0x02-0x03: Content pointer
    row.extend_from_slice(&content_ptr.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(name);
    name_str.write_to(&mut row).expect("write to Vec should not fail");

    row
}

/// Build a menu row (8 bytes fixed size)
///
/// Menu rows define the root navigation items on CDJ/XDJ equipment.
/// Format:
/// - 0x00-0x01: Category ID (references Columns table)
/// - 0x02-0x03: Content pointer (data source)
/// - 0x04: Unknown type
/// - 0x05: Visibility (0=visible, 1=hidden)
/// - 0x06-0x07: Sort order
pub fn build_menu_row(
    category_id: u16,
    content_ptr: u16,
    unknown: u8,
    visibility: u8,
    sort_order: u16,
) -> Vec<u8> {
    let mut row = Vec::with_capacity(8);

    // 0x00-0x01: Category ID (references Columns table)
    row.extend_from_slice(&category_id.to_le_bytes());
    // 0x02-0x03: Content pointer (data source)
    row.extend_from_slice(&content_ptr.to_le_bytes());
    // 0x04: Unknown type
    row.push(unknown);
    // 0x05: Visibility
    row.push(visibility);
    // 0x06-0x07: Sort order
    row.extend_from_slice(&sort_order.to_le_bytes());

    row
}
