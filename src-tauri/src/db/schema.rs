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
    duration_ms INTEGER
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
    PRIMARY KEY (release_id, track_position)
);

CREATE TABLE discovery_sc_client_id_cache (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    client_id  TEXT    NOT NULL,
    fetched_at TEXT    NOT NULL
);
"#,
    ]
}
