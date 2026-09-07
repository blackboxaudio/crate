pub fn get_migrations() -> Vec<&'static str> {
    vec![
        // Migration 1: Initial schema
        r#"
-- Core tables
CREATE TABLE tracks (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT,

    -- Metadata (from ID3/Vorbis)
    title TEXT,
    artist TEXT,
    album TEXT,
    year INTEGER,
    genre TEXT,
    label TEXT,
    catalog_number TEXT,

    -- Audio properties
    duration_ms INTEGER NOT NULL,
    bpm REAL,
    key TEXT,
    bitrate INTEGER,
    sample_rate INTEGER,
    format TEXT,

    -- Analysis metadata
    analysis_source TEXT,
    waveform_data BLOB,

    -- Artwork
    artwork_path TEXT,
    artwork_source TEXT,

    -- User data
    rating INTEGER DEFAULT 0,
    play_count INTEGER DEFAULT 0,
    color TEXT,

    -- Timestamps
    date_added TEXT NOT NULL,
    date_modified TEXT NOT NULL,
    last_played TEXT,

    -- Rekordbox sync
    rekordbox_id TEXT,

    CONSTRAINT valid_rating CHECK (rating >= 0 AND rating <= 5)
);

CREATE INDEX idx_tracks_artist ON tracks(artist);
CREATE INDEX idx_tracks_bpm ON tracks(bpm);
CREATE INDEX idx_tracks_key ON tracks(key);
CREATE INDEX idx_tracks_date_added ON tracks(date_added);
CREATE INDEX idx_tracks_color ON tracks(color);

-- Tag system
CREATE TABLE tag_categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    color TEXT DEFAULT '#6366f1'
);

CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    category_id TEXT NOT NULL REFERENCES tag_categories(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    UNIQUE(category_id, name)
);

CREATE TABLE track_tags (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, tag_id)
);

-- Playlists
CREATE TABLE playlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT REFERENCES playlists(id) ON DELETE CASCADE,
    is_folder INTEGER NOT NULL DEFAULT 0,
    is_smart INTEGER NOT NULL DEFAULT 0,
    smart_rules TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    context TEXT NOT NULL DEFAULT 'library',
    date_created TEXT NOT NULL,
    date_modified TEXT NOT NULL
);

CREATE INDEX idx_playlists_context ON playlists(context);

CREATE TABLE playlist_tracks (
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    date_added TEXT NOT NULL,
    PRIMARY KEY (playlist_id, track_id)
);

-- Cue points
CREATE TABLE cues (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position_ms INTEGER NOT NULL,
    type TEXT NOT NULL,
    loop_end_ms INTEGER,
    hot_cue_index INTEGER,
    name TEXT,
    color TEXT,
    CONSTRAINT valid_type CHECK (type IN ('memory', 'hot', 'loop')),
    CONSTRAINT loop_has_end CHECK (type != 'loop' OR loop_end_ms IS NOT NULL)
);

CREATE INDEX idx_cues_track ON cues(track_id);

-- App settings
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Device export tracking
CREATE TABLE device_exports (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    last_export_at TEXT NOT NULL,
    last_sync_at TEXT,
    sync_enabled INTEGER NOT NULL DEFAULT 1,
    UNIQUE(device_id, playlist_id)
);

CREATE INDEX idx_device_exports_device ON device_exports(device_id);
CREATE INDEX idx_device_exports_playlist ON device_exports(playlist_id);

CREATE TABLE device_tracks (
    device_id TEXT NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    usb_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    pdb_track_id INTEGER,
    metadata_hash TEXT,
    exported_at TEXT NOT NULL,
    PRIMARY KEY (device_id, track_id)
);

CREATE INDEX idx_device_tracks_device ON device_tracks(device_id);

-- Export checkpoints for resume support
CREATE TABLE export_checkpoints (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    state TEXT NOT NULL,
    playlist_ids TEXT NOT NULL,
    tracks_completed TEXT NOT NULL,
    tracks_failed TEXT NOT NULL,
    last_updated_at TEXT NOT NULL
);

CREATE INDEX idx_export_checkpoints_device ON export_checkpoints(device_id);

-- Discovery releases
CREATE TABLE discovery_releases (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    source_type TEXT NOT NULL DEFAULT 'other',
    artist TEXT,
    title TEXT,
    label TEXT,
    release_date TEXT,
    artwork_url TEXT,
    artwork_path TEXT,
    notes TEXT,
    parent_url TEXT,
    date_added TEXT NOT NULL,
    date_modified TEXT NOT NULL
);

CREATE INDEX idx_discovery_releases_date_added ON discovery_releases(date_added);

CREATE TABLE discovery_tracks (
    id TEXT PRIMARY KEY,
    release_id TEXT NOT NULL REFERENCES discovery_releases(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    duration_ms INTEGER,
    video_id TEXT
);

CREATE INDEX idx_discovery_tracks_release ON discovery_tracks(release_id);

CREATE TABLE discovery_release_tags (
    release_id TEXT NOT NULL REFERENCES discovery_releases(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (release_id, tag_id)
);

CREATE TABLE playlist_discovery_releases (
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    release_id TEXT NOT NULL REFERENCES discovery_releases(id) ON DELETE CASCADE,
    position INTEGER,
    date_added TEXT,
    PRIMARY KEY (playlist_id, release_id)
);

-- Stream cache tables for preview playback
CREATE TABLE discovery_stream_cache (
    release_id     TEXT    NOT NULL REFERENCES discovery_releases(id) ON DELETE CASCADE,
    track_position INTEGER NOT NULL,
    stream_url     TEXT    NOT NULL,
    expires_at     TEXT    NOT NULL,
    proxy_ua       TEXT,
    PRIMARY KEY (release_id, track_position)
);

CREATE TABLE discovery_sc_client_id_cache (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    client_id  TEXT    NOT NULL,
    fetched_at TEXT    NOT NULL
);

CREATE TABLE discovery_audio_cache (
    release_id     TEXT    NOT NULL,
    track_position INTEGER NOT NULL,
    content_type   TEXT    NOT NULL DEFAULT 'audio/mpeg',
    file_size      INTEGER NOT NULL,
    cached_at      TEXT    NOT NULL,
    PRIMARY KEY (release_id, track_position)
);
"#,
        // Migration 2: Track-level likes for discovery releases
        r#"
ALTER TABLE discovery_tracks ADD COLUMN is_liked INTEGER NOT NULL DEFAULT 0;
"#,
        // Migration 3: Cloud-sync foundations — HLC columns, track rooting, indexes.
        // `library_roots` is created first so the `tracks.library_root_id` FK resolves.
        // `_hlc TEXT NOT NULL DEFAULT ''` back-fills existing rows with the "never stamped"
        // sentinel (which sorts below every real HLC). A REFERENCES column added via
        // ALTER TABLE must default to NULL (it does), which SQLite permits.
        r#"
CREATE TABLE library_roots (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    _hlc TEXT NOT NULL DEFAULT ''
);

ALTER TABLE tracks                      ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE playlists                   ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE playlist_tracks             ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE cues                        ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE tag_categories              ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE tags                        ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE track_tags                  ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE discovery_releases          ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE discovery_tracks            ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE discovery_release_tags      ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE playlist_discovery_releases ADD COLUMN _hlc TEXT NOT NULL DEFAULT '';

ALTER TABLE tracks ADD COLUMN library_root_id TEXT REFERENCES library_roots(id) ON DELETE SET NULL;
ALTER TABLE tracks ADD COLUMN relative_path   TEXT;

CREATE INDEX idx_tracks_hlc                      ON tracks(_hlc);
CREATE INDEX idx_playlists_hlc                   ON playlists(_hlc);
CREATE INDEX idx_playlist_tracks_hlc             ON playlist_tracks(_hlc);
CREATE INDEX idx_cues_hlc                        ON cues(_hlc);
CREATE INDEX idx_tag_categories_hlc              ON tag_categories(_hlc);
CREATE INDEX idx_tags_hlc                        ON tags(_hlc);
CREATE INDEX idx_track_tags_hlc                  ON track_tags(_hlc);
CREATE INDEX idx_discovery_releases_hlc          ON discovery_releases(_hlc);
CREATE INDEX idx_discovery_tracks_hlc            ON discovery_tracks(_hlc);
CREATE INDEX idx_discovery_release_tags_hlc      ON discovery_release_tags(_hlc);
CREATE INDEX idx_playlist_discovery_releases_hlc ON playlist_discovery_releases(_hlc);
CREATE INDEX idx_library_roots_hlc               ON library_roots(_hlc);
"#,
        // Migration 4: Cloud-sync bookkeeping. These tables are device-local — they are
        // never themselves serialized as sync buckets.
        r#"
-- Per-device mapping from a synced library_root to its local absolute folder.
CREATE TABLE IF NOT EXISTS sync_root_mappings (
    library_root_id     TEXT PRIMARY KEY,
    local_absolute_path TEXT NOT NULL
);

-- Hard-deleted rows to propagate. `entity_id` is the row's PK; composite junction
-- keys are encoded "a|b" in PK-declaration column order (see pipeline::dirty).
CREATE TABLE IF NOT EXISTS sync_tombstones (
    entity_type TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    _hlc        TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_sync_tombstones_hlc ON sync_tombstones(_hlc);

-- Buckets changed locally since the last successful push (drained on push).
CREATE TABLE IF NOT EXISTS sync_dirty_buckets (
    bucket    TEXT PRIMARY KEY,
    marked_at TEXT NOT NULL
);

-- Singleton key/value store for the sync engine (node_id, HLC clock, cursors, flags).
CREATE TABLE IF NOT EXISTS sync_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#,
        // Migration 5: Follow artists & labels.
        // `followed_sources`, `discovery_release_sources`, and the new
        // `discovery_releases` columns (`is_new`, `surfaced_at`) SYNC — they carry
        // `_hlc` and are registered as sync buckets (see pipeline::buckets). The
        // per-device watch bookkeeping (`followed_source_state`,
        // `followed_source_releases`) stays LOCAL and is never serialized as a bucket.
        r#"
-- SYNCED: the artists/labels the user follows.
CREATE TABLE followed_sources (
    id            TEXT PRIMARY KEY,
    url           TEXT NOT NULL UNIQUE,         -- normalize_url()'d page URL
    source_type   TEXT NOT NULL,                -- 'bandcamp' | 'soundcloud' | 'discogs'
    follow_type   TEXT NOT NULL DEFAULT 'artist', -- 'artist' | 'label'
    name          TEXT,
    artwork_url   TEXT,
    artwork_path  TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1,    -- 0 = paused (syncs)
    date_added    TEXT NOT NULL,
    date_modified TEXT NOT NULL,
    _hlc          TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_followed_sources_hlc ON followed_sources(_hlc);

-- LOCAL ONLY: per-device watch bookkeeping for each followed source.
CREATE TABLE followed_source_state (
    source_id            TEXT PRIMARY KEY REFERENCES followed_sources(id) ON DELETE CASCADE,
    last_checked_at      TEXT,
    last_success_at      TEXT,
    health               TEXT NOT NULL DEFAULT 'unknown', -- 'ok'|'error'|'rate_limited'|'unknown'
    last_error           TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    baseline_established INTEGER NOT NULL DEFAULT 0
);

-- LOCAL ONLY: every release URL ever seen under a source, with its disposition.
-- status: 'baseline' (present at follow-time, never surfaced),
--         'surfaced' (auto-added to discovery as new),
--         'dismissed' (user deleted it — tombstone so the watch loop never re-adds).
CREATE TABLE followed_source_releases (
    source_id            TEXT NOT NULL REFERENCES followed_sources(id) ON DELETE CASCADE,
    seen_url             TEXT NOT NULL,          -- normalize_url()'d release URL
    status               TEXT NOT NULL DEFAULT 'baseline',
    release_id           TEXT,                   -- discovery_releases.id when status='surfaced'
    release_day_notified INTEGER NOT NULL DEFAULT 0,
    first_seen_at        TEXT NOT NULL,
    PRIMARY KEY (source_id, seen_url)
);
CREATE INDEX idx_followed_source_releases_status ON followed_source_releases(source_id, status);

-- SYNCED: provenance (many-to-many). A release can be surfaced by an artist follow
-- AND a label follow; it appears once and cites both.
CREATE TABLE discovery_release_sources (
    release_id TEXT NOT NULL REFERENCES discovery_releases(id) ON DELETE CASCADE,
    source_id  TEXT NOT NULL REFERENCES followed_sources(id) ON DELETE CASCADE,
    _hlc       TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (release_id, source_id)
);
CREATE INDEX idx_discovery_release_sources_hlc ON discovery_release_sources(_hlc);

-- SYNCED columns on the existing discovery_releases table: 'new/unreviewed' flag
-- and when the watcher surfaced it (NULL for manually-added releases).
ALTER TABLE discovery_releases ADD COLUMN is_new INTEGER NOT NULL DEFAULT 0;
ALTER TABLE discovery_releases ADD COLUMN surfaced_at TEXT;
"#,
        // discovery_releases.source_page_url — the artist/label page a release was
        // discovered from. Bandcamp label discographies span many artist subdomains, so a
        // release's own URL host isn't the followed page; recording the scanned page lets a
        // label follow match every release imported from it. Synced.
        r#"
ALTER TABLE discovery_releases ADD COLUMN source_page_url TEXT;
"#,
        // Migration 7: LRU eviction for the on-disk audio-byte cache. `last_accessed_at`
        // (RFC 3339) is touched on every cache write and on every playback read so the
        // eviction sweep can drop the least-recently-played tracks once the cache exceeds
        // its size cap. Device-local (the cache itself is never synced). Backfilled from
        // `cached_at` so pre-existing entries have a sensible ordering.
        r#"
ALTER TABLE discovery_audio_cache ADD COLUMN last_accessed_at TEXT;
UPDATE discovery_audio_cache SET last_accessed_at = cached_at WHERE last_accessed_at IS NULL;
"#,
        // Migration 8: on-disk cache for remote discovery artwork, giving mobile offline
        // album art. Keyed by release_id (one cover per release). `last_accessed_at` (RFC
        // 3339) is touched on every render read so the eviction sweep drops the least-
        // recently-shown covers once the cache exceeds its (user-configurable) size cap.
        // Device-local — the cache is never synced.
        r#"
CREATE TABLE discovery_artwork_cache (
    release_id       TEXT    PRIMARY KEY,
    ext              TEXT    NOT NULL DEFAULT 'webp',
    file_size        INTEGER NOT NULL,
    cached_at        TEXT    NOT NULL,
    last_accessed_at TEXT    NOT NULL
);
"#,
        // Migration 9: pinned downloads. `pinned = 1` marks tracks cached via the explicit
        // "Download for Offline" action; the LRU eviction sweep skips them so heavy listening
        // can never silently evict a download (only "Remove Download" / "Clear cache" delete
        // them). Device-local — the cache is never synced.
        r#"
ALTER TABLE discovery_audio_cache ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
"#,
        // Migration 10: per-track page URLs (Bandcamp `/track/...`, SoundCloud permalinks) for
        // track-level share/copy. Nullable — YouTube/Discogs tracks have no page of their own,
        // and pre-existing rows backfill lazily on metadata refresh. Synced (`_hlc` table).
        r#"
ALTER TABLE discovery_tracks ADD COLUMN url TEXT;
"#,
        // Migration 11: purchased-collection accounts (Bandcamp fan pages in v1;
        // `source_type` discriminates future purchase sources). `collection_accounts` and
        // `collection_items` SYNC — they carry `_hlc` and are registered as sync buckets
        // (see pipeline::buckets). Per-device refresh bookkeeping
        // (`collection_account_state`) stays LOCAL, mirroring `followed_source_state`.
        // Ownership of discovery releases/tracks is DERIVED from `collection_items.url`
        // at read time — no ownership columns are added anywhere else.
        r#"
-- SYNCED: linked collection accounts (a Bandcamp fan page URL each).
CREATE TABLE collection_accounts (
    id            TEXT PRIMARY KEY,               -- deterministic v5 of the normalized url
    url           TEXT NOT NULL UNIQUE,           -- normalize_url()'d fan-page URL
    source_type   TEXT NOT NULL DEFAULT 'bandcamp',
    external_id   TEXT,                           -- source-native account id (Bandcamp fan_id)
    username      TEXT,                           -- source-native handle
    name          TEXT,                           -- display name
    avatar_url    TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1,     -- 0 = paused: items stop counting as owned (syncs)
    date_added    TEXT NOT NULL,
    date_modified TEXT NOT NULL,
    _hlc          TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_collection_accounts_hlc ON collection_accounts(_hlc);

-- SYNCED: every owned item across all linked accounts. The same release owned on two
-- accounts is two rows (ids encode account|url) so unlinking one account never deletes
-- the other's copy; reads union by url.
CREATE TABLE collection_items (
    id            TEXT PRIMARY KEY,               -- deterministic v5 of "account_id|normalized url"
    account_id    TEXT NOT NULL REFERENCES collection_accounts(id) ON DELETE CASCADE,
    source_type   TEXT NOT NULL DEFAULT 'bandcamp',
    item_type     TEXT NOT NULL,                  -- 'album' | 'track'
    url           TEXT NOT NULL,                  -- normalize_url()'d item page URL (the match key)
    external_id   TEXT,                           -- source-native item id
    artist        TEXT,
    title         TEXT,
    artwork_url   TEXT,
    purchased_at  TEXT,                           -- RFC 3339 when the source date parses
    date_added    TEXT NOT NULL,
    date_modified TEXT NOT NULL,
    _hlc          TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_collection_items_hlc ON collection_items(_hlc);
CREATE INDEX idx_collection_items_account ON collection_items(account_id);
CREATE INDEX idx_collection_items_url ON collection_items(url);

-- LOCAL ONLY: per-device refresh bookkeeping for each linked account.
CREATE TABLE collection_account_state (
    account_id           TEXT PRIMARY KEY REFERENCES collection_accounts(id) ON DELETE CASCADE,
    last_checked_at      TEXT,
    last_success_at      TEXT,
    health               TEXT NOT NULL DEFAULT 'unknown', -- 'ok'|'error'|'rate_limited'|'unknown'
    last_error           TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_item_count      INTEGER
);
"#,
        // Migration 12: per-track preview availability. A row means the source currently
        // serves NO stream for that track position (Bandcamp pre-order albums stream only
        // the featured single; the rest are unreleased). Rows are replaced wholesale per
        // release on every successful stream extraction, so a released album self-heals on
        // the next check. Derived source state, re-fetchable anywhere — LOCAL, never synced.
        r#"
CREATE TABLE discovery_preview_unavailable (
    release_id TEXT    NOT NULL,
    position   INTEGER NOT NULL,
    checked_at TEXT    NOT NULL,
    PRIMARY KEY (release_id, position)
);
"#,
        // Migration 13: whether the last collection walk covered the whole collection.
        // The incremental scrape's stop-on-all-known rule assumes stored items form a
        // newest-first prefix; an interrupted initial walk breaks that (newest items
        // known, older never fetched) and every later incremental sync would exit on
        // page 1 forever. Defaults 0 so existing wedged accounts heal with one full
        // walk on their next refresh. LOCAL, never synced.
        r#"
ALTER TABLE collection_account_state ADD COLUMN last_walk_complete INTEGER NOT NULL DEFAULT 0;
"#,
        // Migration 14: when a discovery track was (last) liked, so the liked pool can be
        // sorted by like recency. Nullable RFC3339: NULL for unliked rows and for likes
        // that predate this column (they sort last rather than getting a fake date).
        // Cleared on unlike so re-liking records a fresh date. Synced — rides the row's
        // `_hlc` with `is_liked`, so the pair never splits under whole-row LWW.
        r#"
ALTER TABLE discovery_tracks ADD COLUMN liked_at TEXT;
"#,
    ]
}
