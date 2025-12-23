//! PDB table types and row builders
//!
//! This module contains the table type enums and row building functions
//! for all PDB table types.

pub mod album;
pub mod artist;
pub mod color;
pub mod genre;
pub mod key;
pub mod menu;
pub mod playlist;
pub mod tag;
pub mod track;

pub use album::*;
pub use artist::*;
pub use color::*;
pub use genre::*;
pub use key::*;
pub use menu::*;
pub use playlist::*;
pub use tag::*;
pub use track::*;

/// Table types in the PDB format (export.pdb)
///
/// All 20 types (0-19) must be present in numeric order in the file header.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableType {
    Tracks = 0,
    Genres = 1,
    Artists = 2,
    Albums = 3,
    Labels = 4,
    Keys = 5,
    Colors = 6,
    PlaylistTree = 7,
    PlaylistEntries = 8,
    Unknown9 = 9,
    Unknown10 = 10,
    HistoryPlaylists = 11,
    HistoryEntries = 12,
    Artwork = 13,
    Unknown14 = 14,
    Unknown15 = 15,
    Columns = 16,
    Menu = 17,
    Unknown18 = 18,
    History = 19,
}

impl TableType {
    /// Get all required table types in order
    pub fn all_required() -> &'static [TableType] {
        &[
            TableType::Tracks,
            TableType::Genres,
            TableType::Artists,
            TableType::Albums,
            TableType::Labels,
            TableType::Keys,
            TableType::Colors,
            TableType::PlaylistTree,
            TableType::PlaylistEntries,
            TableType::Unknown9,
            TableType::Unknown10,
            TableType::HistoryPlaylists,
            TableType::HistoryEntries,
            TableType::Artwork,
            TableType::Unknown14,
            TableType::Unknown15,
            TableType::Columns,
            TableType::Menu,
            TableType::Unknown18,
            TableType::History,
        ]
    }

    /// Check if this table type uses the Menu/Columns data page header format
    pub fn uses_menu_header(&self) -> bool {
        matches!(self, TableType::Menu | TableType::Columns)
    }
}

impl From<TableType> for u32 {
    fn from(t: TableType) -> u32 {
        t as u32
    }
}

impl TryFrom<u32> for TableType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TableType::Tracks),
            1 => Ok(TableType::Genres),
            2 => Ok(TableType::Artists),
            3 => Ok(TableType::Albums),
            4 => Ok(TableType::Labels),
            5 => Ok(TableType::Keys),
            6 => Ok(TableType::Colors),
            7 => Ok(TableType::PlaylistTree),
            8 => Ok(TableType::PlaylistEntries),
            9 => Ok(TableType::Unknown9),
            10 => Ok(TableType::Unknown10),
            11 => Ok(TableType::HistoryPlaylists),
            12 => Ok(TableType::HistoryEntries),
            13 => Ok(TableType::Artwork),
            14 => Ok(TableType::Unknown14),
            15 => Ok(TableType::Unknown15),
            16 => Ok(TableType::Columns),
            17 => Ok(TableType::Menu),
            18 => Ok(TableType::Unknown18),
            19 => Ok(TableType::History),
            _ => Err(()),
        }
    }
}

/// Extended table types for exportExt.pdb
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtTableType {
    Unknown0 = 0,
    Unknown1 = 1,
    Unknown2 = 2,
    Tags = 3,
    TrackTags = 4,
}

impl ExtTableType {
    /// Get all required extended table types in order
    pub fn all_required() -> &'static [ExtTableType] {
        &[
            ExtTableType::Unknown0,
            ExtTableType::Unknown1,
            ExtTableType::Unknown2,
            ExtTableType::Tags,
            ExtTableType::TrackTags,
        ]
    }
}

impl From<ExtTableType> for u32 {
    fn from(t: ExtTableType) -> u32 {
        t as u32
    }
}

impl TryFrom<u32> for ExtTableType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ExtTableType::Unknown0),
            1 => Ok(ExtTableType::Unknown1),
            2 => Ok(ExtTableType::Unknown2),
            3 => Ok(ExtTableType::Tags),
            4 => Ok(ExtTableType::TrackTags),
            _ => Err(()),
        }
    }
}
