mod crud;
mod expansion;
mod movement;
mod releases;
mod smart;
mod tracks;

pub use expansion::{expand_release_memberships, expand_release_memberships_for};

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryRelease, DiscoveryTrack, MoveConflict, MovePlaylistResult, Playlist, PlaylistCoverArt,
    SmartRules, Tag, Track,
};
use crate::services::smart_rules;

pub struct PlaylistService {
    conn: Arc<Mutex<Connection>>,
}

impl PlaylistService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}
