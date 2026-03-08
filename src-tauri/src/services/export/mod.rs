pub mod anlz;
pub mod checkpoint;
pub mod device_library_plus;
pub mod pdb;
pub mod sync_diff;

mod cleanup;
mod collection;
mod copy;
mod generation;
mod helpers;
mod orchestration;
mod state;
mod validation;

pub use checkpoint::CheckpointService;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{
    Cue, CueType, DeviceExport, DeviceTrack, ExportProgress, ExportRequest, ExportResult, Playlist,
    Track,
};

use self::device_library_plus::{
    Content, Cue as DlpCue, CueKind, DeviceLibraryPlusWriter, FileType, Playlist as DlpPlaylist,
    PlaylistType, Property,
};
use self::helpers::{build_usb_path, cleanup_empty_dirs, get_all_descendant_playlist_ids_impl};
use self::pdb::RekordboxPdbWriter;

/// Service for exporting playlists to USB devices in Rekordbox-compatible format
pub struct ExportService {
    conn: Arc<Mutex<Connection>>,
    /// Flag to signal export cancellation
    cancel_flag: Arc<AtomicBool>,
}

impl ExportService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Request cancellation of the current export
    pub fn cancel_export(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }
}
