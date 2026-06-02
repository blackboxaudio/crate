//! Bucket identity — the canonical names of the per-entity sync buckets, and the
//! `tracks` shard function. Phase 1 extends this module with JSONL serialize/parse
//! and the merge entry points; for now it only names buckets and shards tracks.
//!
//! These plain names are the manifest map keys, the `sync_dirty_buckets.bucket`
//! values, and (in Phase 2) the Cloud Storage object filenames inside the single
//! `crate-sync` bucket: `users/{uid}/vault/{bucket}-{blake3}.jsonl.gz`.

/// Number of hash-shards the `tracks` bucket is split into.
pub const TRACK_SHARDS: usize = 16;

/// Logical entity name for track tombstones. Track *data* lives in the sharded
/// `tracks/0`..`tracks/f` buckets, but a deleted track is recorded once under
/// this single entity type; the shard serializer maps it back via
/// [`bucket_for_track_id`].
pub const TRACKS_ENTITY: &str = "tracks";

pub const PLAYLISTS: &str = "playlists";
pub const PLAYLIST_TRACKS: &str = "playlist_tracks";
pub const CUES: &str = "cues";
pub const TAG_CATEGORIES: &str = "tag_categories";
pub const TAGS: &str = "tags";
pub const TRACK_TAGS: &str = "track_tags";
pub const DISCOVERY_RELEASES: &str = "discovery_releases";
pub const DISCOVERY_TRACKS: &str = "discovery_tracks";
pub const DISCOVERY_RELEASE_TAGS: &str = "discovery_release_tags";
pub const PLAYLIST_DISCOVERY_RELEASES: &str = "playlist_discovery_releases";
pub const LIBRARY_ROOTS: &str = "library_roots";
pub const SETTINGS: &str = "settings";

/// Canonical bucket name for a track id, e.g. `"tracks/3"`. Sharded by the first
/// hex char of the (UUID) id so a single-track edit re-uploads ~1/16th of the
/// library instead of the whole `tracks` blob.
pub fn bucket_for_track_id(id: &str) -> String {
    let c = id
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| c.is_ascii_hexdigit())
        .unwrap_or('0');
    format!("tracks/{c}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shards_by_first_hex_char() {
        assert_eq!(bucket_for_track_id("a1b2c3d4-..."), "tracks/a");
        assert_eq!(bucket_for_track_id("0fffffff-..."), "tracks/0");
        // Non-hex / empty fall back to shard 0.
        assert_eq!(bucket_for_track_id(""), "tracks/0");
        assert_eq!(bucket_for_track_id("zzz"), "tracks/0");
    }
}
