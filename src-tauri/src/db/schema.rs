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

    -- User data
    rating INTEGER DEFAULT 0,
    play_count INTEGER DEFAULT 0,

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

-- Tag system
CREATE TABLE tag_categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL DEFAULT 0
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
    date_created TEXT NOT NULL,
    date_modified TEXT NOT NULL
);

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
"#,
        // Migration 2: Add color to tag_categories
        r#"
ALTER TABLE tag_categories ADD COLUMN color TEXT DEFAULT '#6366f1';
"#,
        // Migration 3: Add artwork_path to tracks
        r#"
ALTER TABLE tracks ADD COLUMN artwork_path TEXT;
"#,
        // Migration 4: Add color to tracks (Rekordbox-compatible track colors)
        r#"
ALTER TABLE tracks ADD COLUMN color TEXT;
CREATE INDEX idx_tracks_color ON tracks(color);
"#,
        // Migration 5: Add artwork_source to tracks (for tracking extracted vs user-provided artwork)
        r#"
ALTER TABLE tracks ADD COLUMN artwork_source TEXT;
-- Values: 'extracted', 'user_provided', or NULL
"#,
        // Migration 6: Device export tracking for USB sync
        r#"
-- Track which playlists have been exported to which devices
CREATE TABLE device_exports (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,           -- Volume UUID (stable across reconnections)
    device_name TEXT NOT NULL,         -- Human-readable device name
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    last_export_at TEXT NOT NULL,      -- ISO timestamp
    sync_enabled INTEGER NOT NULL DEFAULT 1,
    UNIQUE(device_id, playlist_id)
);

-- Track which files have been copied to which devices
CREATE TABLE device_tracks (
    device_id TEXT NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    usb_path TEXT NOT NULL,            -- Path on USB relative to Contents/
    file_hash TEXT NOT NULL,           -- Hash at time of export
    pdb_track_id INTEGER,              -- Sequential ID assigned in PDB
    exported_at TEXT NOT NULL,
    PRIMARY KEY (device_id, track_id)
);

CREATE INDEX idx_device_exports_device ON device_exports(device_id);
CREATE INDEX idx_device_exports_playlist ON device_exports(playlist_id);
CREATE INDEX idx_device_tracks_device ON device_tracks(device_id);
"#,
        // Migration 7: Add last_sync_at for tracking sync timestamps
        r#"
ALTER TABLE device_exports ADD COLUMN last_sync_at TEXT;
"#,
    ]
}
