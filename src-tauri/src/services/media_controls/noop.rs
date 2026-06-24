//! No-op media-session backend for platforms without a native one (Android today, and flagless
//! host/test builds). Keeps the `update_now_playing` / `update_playback_state` / `clear_now_playing`
//! commands callable everywhere — they simply do nothing here.

use std::time::Duration;

use super::{MediaSession, NowPlayingMetadata, PlaybackStatus};

pub struct NoopMediaSession;

impl MediaSession for NoopMediaSession {
    fn set_metadata(&self, _meta: &NowPlayingMetadata) {}
    fn set_playback(&self, _status: PlaybackStatus, _progress: Option<Duration>) {}
    fn clear(&self) {}
}
