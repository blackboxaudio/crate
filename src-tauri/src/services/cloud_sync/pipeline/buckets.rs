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

/// The nibble (0..16) a track id shards into — the `u8` payload of
/// [`Bucket::Tracks`]. Mirrors [`bucket_for_track_id`] exactly (first hex char,
/// non-hex/empty → 0) so the enum and the string form never disagree.
pub fn shard_for_track_id(id: &str) -> u8 {
    id.chars()
        .next()
        .and_then(|c| c.to_ascii_lowercase().to_digit(16))
        .unwrap_or(0) as u8
}

/// How a bucket merges. Drives the delete-vs-live tie-break, which MUST be the
/// same at serialize time and merge time or two converged devices produce
/// different bytes (hash ping-pong).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketKind {
    /// Single-PK entity. DELETE-WINS-TIE: a delete at an HLC `>=` the live row wins.
    Entity,
    /// Composite-PK junction. ADD-WINS-TIE: a delete wins only if strictly `>` the add.
    Junction,
    /// `settings` key/value, per-key HLC in `sync_state`. LWW, never deleted.
    Settings,
}

/// A logical sync bucket: one content-addressed blob in the vault. The `tracks`
/// table is split into 16 shards (`Tracks(0)`..`Tracks(15)` → `"tracks/0"`..
/// `"tracks/f"`); every other table is a single whole-file bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Track shard; the `u8` is the nibble 0..16 (NOT the hex char).
    Tracks(u8),
    Playlists,
    PlaylistTracks,
    Cues,
    TagCategories,
    Tags,
    TrackTags,
    DiscoveryReleases,
    DiscoveryTracks,
    DiscoveryReleaseTags,
    PlaylistDiscoveryReleases,
    LibraryRoots,
    Settings,
}

impl Bucket {
    /// The bucket for a given track id (its shard).
    pub fn for_track_id(id: &str) -> Bucket {
        Bucket::Tracks(shard_for_track_id(id))
    }

    /// Canonical plain name — the manifest map key / dirty-queue value.
    /// `Tracks(10)` → `"tracks/a"`.
    pub fn as_str(&self) -> String {
        match self {
            Bucket::Tracks(n) => format!("tracks/{n:x}"),
            Bucket::Playlists => PLAYLISTS.to_string(),
            Bucket::PlaylistTracks => PLAYLIST_TRACKS.to_string(),
            Bucket::Cues => CUES.to_string(),
            Bucket::TagCategories => TAG_CATEGORIES.to_string(),
            Bucket::Tags => TAGS.to_string(),
            Bucket::TrackTags => TRACK_TAGS.to_string(),
            Bucket::DiscoveryReleases => DISCOVERY_RELEASES.to_string(),
            Bucket::DiscoveryTracks => DISCOVERY_TRACKS.to_string(),
            Bucket::DiscoveryReleaseTags => DISCOVERY_RELEASE_TAGS.to_string(),
            Bucket::PlaylistDiscoveryReleases => PLAYLIST_DISCOVERY_RELEASES.to_string(),
            Bucket::LibraryRoots => LIBRARY_ROOTS.to_string(),
            Bucket::Settings => SETTINGS.to_string(),
        }
    }

    /// Parse a plain bucket name. `"tracks/a"` → `Tracks(10)`; rejects `"tracks/ab"`.
    pub fn parse(s: &str) -> Option<Bucket> {
        if let Some(rest) = s.strip_prefix("tracks/") {
            let mut chars = rest.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None; // more than one char after the slash
            }
            let n = c.to_digit(16)? as u8;
            return Some(Bucket::Tracks(n));
        }
        Some(match s {
            PLAYLISTS => Bucket::Playlists,
            PLAYLIST_TRACKS => Bucket::PlaylistTracks,
            CUES => Bucket::Cues,
            TAG_CATEGORIES => Bucket::TagCategories,
            TAGS => Bucket::Tags,
            TRACK_TAGS => Bucket::TrackTags,
            DISCOVERY_RELEASES => Bucket::DiscoveryReleases,
            DISCOVERY_TRACKS => Bucket::DiscoveryTracks,
            DISCOVERY_RELEASE_TAGS => Bucket::DiscoveryReleaseTags,
            PLAYLIST_DISCOVERY_RELEASES => Bucket::PlaylistDiscoveryReleases,
            LIBRARY_ROOTS => Bucket::LibraryRoots,
            SETTINGS => Bucket::Settings,
            _ => return None,
        })
    }

    /// Every bucket, in a stable deterministic order (16 track shards then the
    /// rest). Used for full-library serialization/manifest iteration — this is
    /// NOT the FK-safe merge order (see [`Bucket::merge_order`]).
    pub fn all() -> Vec<Bucket> {
        let mut v: Vec<Bucket> = (0u8..TRACK_SHARDS as u8).map(Bucket::Tracks).collect();
        v.extend([
            Bucket::Playlists,
            Bucket::PlaylistTracks,
            Bucket::Cues,
            Bucket::TagCategories,
            Bucket::Tags,
            Bucket::TrackTags,
            Bucket::DiscoveryReleases,
            Bucket::DiscoveryTracks,
            Bucket::DiscoveryReleaseTags,
            Bucket::PlaylistDiscoveryReleases,
            Bucket::LibraryRoots,
            Bucket::Settings,
        ]);
        v
    }

    /// FK-safe merge/apply order: parents before children. Pulls merge buckets in
    /// this order so a junction's endpoints already exist when it is applied.
    pub fn merge_order() -> Vec<Bucket> {
        let mut v = vec![
            // rank 0 — no synced-parent dependency
            Bucket::TagCategories,
            Bucket::LibraryRoots,
            Bucket::DiscoveryReleases,
        ];
        // rank 1 — depend only on rank 0
        v.extend((0u8..TRACK_SHARDS as u8).map(Bucket::Tracks));
        v.extend([Bucket::Tags, Bucket::Playlists, Bucket::DiscoveryTracks]);
        // rank 2 — children / junctions
        v.extend([
            Bucket::Cues,
            Bucket::TrackTags,
            Bucket::PlaylistTracks,
            Bucket::DiscoveryReleaseTags,
            Bucket::PlaylistDiscoveryReleases,
        ]);
        // rank 3 — independent
        v.push(Bucket::Settings);
        v
    }

    pub fn kind(&self) -> BucketKind {
        match self {
            Bucket::PlaylistTracks
            | Bucket::TrackTags
            | Bucket::DiscoveryReleaseTags
            | Bucket::PlaylistDiscoveryReleases => BucketKind::Junction,
            Bucket::Settings => BucketKind::Settings,
            _ => BucketKind::Entity,
        }
    }

    /// The SQL table backing this bucket. All `Tracks(_)` shards share `"tracks"`.
    pub fn table(&self) -> &'static str {
        match self {
            Bucket::Tracks(_) => "tracks",
            Bucket::Playlists => "playlists",
            Bucket::PlaylistTracks => "playlist_tracks",
            Bucket::Cues => "cues",
            Bucket::TagCategories => "tag_categories",
            Bucket::Tags => "tags",
            Bucket::TrackTags => "track_tags",
            Bucket::DiscoveryReleases => "discovery_releases",
            Bucket::DiscoveryTracks => "discovery_tracks",
            Bucket::DiscoveryReleaseTags => "discovery_release_tags",
            Bucket::PlaylistDiscoveryReleases => "playlist_discovery_releases",
            Bucket::LibraryRoots => "library_roots",
            Bucket::Settings => "settings",
        }
    }

    /// The `sync_tombstones.entity_type` for this bucket. CRITICAL: every track
    /// shard maps to [`TRACKS_ENTITY`] (`"tracks"`), matching how `delete_tracks`
    /// records them — NOT `"tracks/3"`.
    pub fn entity_type(&self) -> &'static str {
        match self {
            Bucket::Tracks(_) => TRACKS_ENTITY,
            Bucket::Playlists => PLAYLISTS,
            Bucket::PlaylistTracks => PLAYLIST_TRACKS,
            Bucket::Cues => CUES,
            Bucket::TagCategories => TAG_CATEGORIES,
            Bucket::Tags => TAGS,
            Bucket::TrackTags => TRACK_TAGS,
            Bucket::DiscoveryReleases => DISCOVERY_RELEASES,
            Bucket::DiscoveryTracks => DISCOVERY_TRACKS,
            Bucket::DiscoveryReleaseTags => DISCOVERY_RELEASE_TAGS,
            Bucket::PlaylistDiscoveryReleases => PLAYLIST_DISCOVERY_RELEASES,
            Bucket::LibraryRoots => LIBRARY_ROOTS,
            Bucket::Settings => SETTINGS,
        }
    }

    /// The display-label column (`name`/`title`) for an entity, used only to name the
    /// entity in an override toast. Meaningless for junctions/settings (which never
    /// produce overrides), where it defaults to `"name"`.
    pub fn label_column(&self) -> &'static str {
        match self {
            Bucket::Tracks(_) | Bucket::DiscoveryReleases => "title",
            _ => "name",
        }
    }

    /// PK column names in declaration order: one element for entities, two for
    /// junctions (matching the `a|b` order of [`super::dirty::junction_entity_id`]),
    /// and `["key"]` for settings.
    pub fn pk_columns(&self) -> &'static [&'static str] {
        match self {
            Bucket::PlaylistTracks => &["playlist_id", "track_id"],
            Bucket::TrackTags => &["track_id", "tag_id"],
            Bucket::DiscoveryReleaseTags => &["release_id", "tag_id"],
            Bucket::PlaylistDiscoveryReleases => &["playlist_id", "release_id"],
            Bucket::Settings => &["key"],
            _ => &["id"],
        }
    }
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

    #[test]
    fn shard_nibble_matches_string_form() {
        for id in ["a1b2", "0fff", "ffff", "5abc", "deadbeef"] {
            let n = shard_for_track_id(id);
            assert_eq!(Bucket::Tracks(n).as_str(), bucket_for_track_id(id));
            assert_eq!(Bucket::for_track_id(id), Bucket::Tracks(n));
        }
        assert_eq!(shard_for_track_id(""), 0);
        assert_eq!(shard_for_track_id("zzz"), 0);
    }

    #[test]
    fn as_str_parse_roundtrip_for_every_bucket() {
        for b in Bucket::all() {
            assert_eq!(Bucket::parse(&b.as_str()), Some(b), "roundtrip {b:?}");
        }
    }

    #[test]
    fn all_shards_format_correctly() {
        assert_eq!(Bucket::Tracks(0).as_str(), "tracks/0");
        assert_eq!(Bucket::Tracks(10).as_str(), "tracks/a");
        assert_eq!(Bucket::Tracks(15).as_str(), "tracks/f");
        assert_eq!(Bucket::parse("tracks/a"), Some(Bucket::Tracks(10)));
        assert_eq!(Bucket::parse("tracks/ab"), None);
        assert_eq!(Bucket::parse("tracks/"), None);
        assert_eq!(Bucket::parse("bogus"), None);
    }

    #[test]
    fn bucket_count_is_28() {
        assert_eq!(Bucket::all().len(), 28); // 16 shards + 12
        assert_eq!(Bucket::merge_order().len(), 28);
    }

    #[test]
    fn kinds_are_correct() {
        assert_eq!(Bucket::Tracks(3).kind(), BucketKind::Entity);
        assert_eq!(Bucket::PlaylistTracks.kind(), BucketKind::Junction);
        assert_eq!(Bucket::TrackTags.kind(), BucketKind::Junction);
        assert_eq!(Bucket::Settings.kind(), BucketKind::Settings);
        assert_eq!(Bucket::Tracks(3).entity_type(), "tracks");
    }
}
