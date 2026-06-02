//! Per-bucket JSONL serialization and parsing — the wire format.
//!
//! ## Determinism (hard invariants — two converged devices MUST produce byte-identical bucket files)
//!
//! - **Serialize only through `#[derive(Serialize)]` structs**, never through
//!   `serde_json::Value`/`json!` (a `Value::Object` serializes keys in *sorted*
//!   order, a struct in *declaration* order — mixing the two yields divergent
//!   bytes). The parse path may use `Value` because it never re-emits.
//! - **Live rows** are wrapped in [`Live`] (entity fields, then `_hlc`, then
//!   `_deleted:false`). **Tombstone rows** carry only their identity columns +
//!   `_hlc` + `_deleted:true`, via dedicated tombstone structs.
//! - **Rows are emitted in canonical-id order** (a `BTreeMap` key sort); junction
//!   ids are `"a|b"` from [`dirty::junction_entity_id`].
//! - **The tombstone-vs-live tie at serialize time is parameterized by
//!   [`BucketKind`]** and MUST match the merge engine: entities resolve a tie to
//!   the tombstone (`>=`), junctions to the live/add (`>`). See [`super::merge`].
//! - Numbers/bools/dates are read into typed fields (`Option<f64>`, `bool`,
//!   verbatim RFC3339 `String`) so serde_json emits them identically every time.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{CrateError, Result};
use crate::models::{
    BackupDiscoveryReleaseTag, BackupPlaylistDiscoveryRelease, BackupTrack, BackupTrackTag,
    DiscoveryTrack, PlaylistTrack, Tag,
};

use super::buckets::{shard_for_track_id, Bucket, BucketKind};
use super::dirty;

// ---------------------------------------------------------------------------
// Wire structs for the entities whose canonical model carries derived/nested
// fields that must NOT reach the wire (`Playlist.track_count`, `TagCategory.tags`,
// `DiscoveryRelease.tracks/tags`). The clean models (`BackupTrack`, `Tag`,
// `DiscoveryTrack`, `PlaylistTrack`, the `Backup*` junctions) are reused directly.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistRow {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub is_smart: bool,
    pub smart_rules: Option<String>,
    pub sort_order: i32,
    pub context: String,
    pub date_created: String,
    pub date_modified: String,
}

/// `cues` wire row. `cue_type` holds the raw `type` column value
/// (`"memory"`/`"hot"`/`"loop"`) — a plain `String`, so neither the serialize nor
/// the merge path has to round-trip `CueType`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueRow {
    pub id: String,
    pub track_id: String,
    pub position_ms: i64,
    pub cue_type: String,
    pub loop_end_ms: Option<i64>,
    pub hot_cue_index: Option<i32>,
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCategoryRow {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReleaseRow {
    pub id: String,
    pub url: String,
    pub source_type: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
    pub artwork_path: Option<String>,
    pub notes: Option<String>,
    pub parent_url: Option<String>,
    pub date_added: String,
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRootRow {
    pub id: String,
    pub name: String,
}

/// `settings` wire row — `{ key, value, _hlc }`. Settings are never deleted, so
/// there is no `_deleted` field and no tombstone path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingRow {
    pub key: String,
    pub value: String,
    #[serde(rename = "_hlc")]
    pub hlc: String,
}

// ---------------------------------------------------------------------------
// Envelopes
// ---------------------------------------------------------------------------

/// A live row: the entity's fields (flattened, declaration order) followed by
/// `_hlc` and `_deleted:false`.
#[derive(Serialize)]
struct Live<T: Serialize> {
    #[serde(flatten)]
    entity: T,
    #[serde(rename = "_hlc")]
    hlc: String,
    #[serde(rename = "_deleted")]
    deleted: bool,
}

#[derive(Serialize)]
struct EntityTombstone<'a> {
    id: &'a str,
    #[serde(rename = "_hlc")]
    hlc: &'a str,
    #[serde(rename = "_deleted")]
    deleted: bool,
}

#[derive(Serialize)]
struct PlaylistTrackTombstone<'a> {
    playlist_id: &'a str,
    track_id: &'a str,
    #[serde(rename = "_hlc")]
    hlc: &'a str,
    #[serde(rename = "_deleted")]
    deleted: bool,
}

#[derive(Serialize)]
struct TrackTagTombstone<'a> {
    track_id: &'a str,
    tag_id: &'a str,
    #[serde(rename = "_hlc")]
    hlc: &'a str,
    #[serde(rename = "_deleted")]
    deleted: bool,
}

#[derive(Serialize)]
struct DiscoveryReleaseTagTombstone<'a> {
    release_id: &'a str,
    tag_id: &'a str,
    #[serde(rename = "_hlc")]
    hlc: &'a str,
    #[serde(rename = "_deleted")]
    deleted: bool,
}

#[derive(Serialize)]
struct PlaylistDiscoveryReleaseTombstone<'a> {
    playlist_id: &'a str,
    release_id: &'a str,
    #[serde(rename = "_hlc")]
    hlc: &'a str,
    #[serde(rename = "_deleted")]
    deleted: bool,
}

/// One parsed wire row. The full object is kept as a `serde_json::Value` so the
/// merge writer can lazily deserialize only the live rows it actually applies.
pub struct ParsedRow {
    pub value: serde_json::Value,
    pub hlc: String,
    pub deleted: bool,
}

/// What a canonical id resolves to during serialization.
enum Emit<T> {
    Live(T, String),
    Dead(String),
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// blake3 hex of the (uncompressed) bucket JSONL. The content-address used by the
/// manifest. NEVER hash gzip output (zlib is non-deterministic across versions).
pub fn bucket_hash(jsonl: &[u8]) -> String {
    blake3::hash(jsonl).to_hex().to_string()
}

/// Serialize a bucket to canonical, uncompressed JSONL: live rows merged with this
/// bucket's tombstones by canonical id, ordered by canonical id.
pub fn serialize_bucket(conn: &Connection, bucket: &Bucket) -> Result<Vec<u8>> {
    let tombs = match bucket.kind() {
        BucketKind::Settings => return serialize_settings(conn),
        _ => read_tombstones(conn, bucket)?,
    };
    match bucket {
        Bucket::Tracks(_) => emit(bucket, read_live_tracks(conn, bucket)?, tombs),
        Bucket::Playlists => emit(bucket, read_live_playlists(conn)?, tombs),
        Bucket::PlaylistTracks => emit(bucket, read_live_playlist_tracks(conn)?, tombs),
        Bucket::Cues => emit(bucket, read_live_cues(conn)?, tombs),
        Bucket::TagCategories => emit(bucket, read_live_tag_categories(conn)?, tombs),
        Bucket::Tags => emit(bucket, read_live_tags(conn)?, tombs),
        Bucket::TrackTags => emit(bucket, read_live_track_tags(conn)?, tombs),
        Bucket::DiscoveryReleases => emit(bucket, read_live_discovery_releases(conn)?, tombs),
        Bucket::DiscoveryTracks => emit(bucket, read_live_discovery_tracks(conn)?, tombs),
        Bucket::DiscoveryReleaseTags => {
            emit(bucket, read_live_discovery_release_tags(conn)?, tombs)
        }
        Bucket::PlaylistDiscoveryReleases => {
            emit(bucket, read_live_playlist_discovery_releases(conn)?, tombs)
        }
        Bucket::LibraryRoots => emit(bucket, read_live_library_roots(conn)?, tombs),
        Bucket::Settings => unreachable!("handled above"),
    }
}

/// Parse a bucket's JSONL into rows. Lenient: extra fields are ignored; each row
/// is validated to carry its PK column(s) via [`canonical_id`].
pub fn parse_bucket(bucket: &Bucket, bytes: &[u8]) -> Result<Vec<ParsedRow>> {
    let mut out = Vec::new();
    for line in bytes.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(line)
            .map_err(|e| CrateError::CloudSync(format!("bad JSONL row: {e}")))?;
        // Validate identity is present (also rejects junk lines early).
        let _ = canonical_id(bucket, &value)?;
        let hlc = value
            .get("_hlc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let deleted = value
            .get("_deleted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        out.push(ParsedRow {
            value,
            hlc,
            deleted,
        });
    }
    Ok(out)
}

/// Canonical identity for a parsed row: the `id` (or `key`) for entities/settings,
/// or `junction_entity_id(pk0, pk1)` for junctions.
pub fn canonical_id(bucket: &Bucket, value: &serde_json::Value) -> Result<String> {
    let get = |col: &str| -> Result<String> {
        value
            .get(col)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                CrateError::CloudSync(format!(
                    "row missing PK column {col:?} in bucket {}",
                    bucket.as_str()
                ))
            })
    };
    match bucket.pk_columns() {
        [a] => get(a),
        [a, b] => Ok(dirty::junction_entity_id(&get(a)?, &get(b)?)),
        _ => Err(CrateError::CloudSync(
            "bucket has unexpected PK arity".into(),
        )),
    }
}

/// Max `_hlc` across a bucket's live rows and tombstones, `""` if none. Used as the
/// manifest's per-bucket watermark (diagnostic; diffing keys on the blob hash).
pub fn bucket_max_hlc(conn: &Connection, bucket: &Bucket) -> Result<String> {
    if bucket.kind() == BucketKind::Settings {
        let v: Option<String> = conn.query_row(
            "SELECT MAX(value) FROM sync_state WHERE key LIKE 'setting_hlc:%'",
            [],
            |r| r.get::<_, Option<String>>(0),
        )?;
        return Ok(v.unwrap_or_default());
    }

    if let Bucket::Tracks(n) = bucket {
        let mut mx = String::new();
        let mut stmt = conn.prepare("SELECT id, _hlc FROM tracks")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, hlc) = row?;
            if shard_for_track_id(&id) == *n && hlc > mx {
                mx = hlc;
            }
        }
        let mut stmt = conn
            .prepare("SELECT entity_id, _hlc FROM sync_tombstones WHERE entity_type = 'tracks'")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (eid, hlc) = row?;
            if shard_for_track_id(&eid) == *n && hlc > mx {
                mx = hlc;
            }
        }
        return Ok(mx);
    }

    let table = bucket.table();
    let live: Option<String> =
        conn.query_row(&format!("SELECT MAX(_hlc) FROM {table}"), [], |r| {
            r.get::<_, Option<String>>(0)
        })?;
    let tomb: Option<String> = conn.query_row(
        "SELECT MAX(_hlc) FROM sync_tombstones WHERE entity_type = ?1",
        [bucket.entity_type()],
        |r| r.get::<_, Option<String>>(0),
    )?;
    Ok([live, tomb].into_iter().flatten().max().unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

fn emit<T: Serialize>(
    bucket: &Bucket,
    live: Vec<(String, T, String)>,
    tombs: Vec<(String, String)>,
) -> Result<Vec<u8>> {
    let tie_to_delete = bucket.kind() == BucketKind::Entity;

    let mut map: BTreeMap<String, Emit<T>> = BTreeMap::new();
    for (cid, entity, hlc) in live {
        map.insert(cid, Emit::Live(entity, hlc));
    }
    for (cid, h_tomb) in tombs {
        let replace = match map.get(&cid) {
            Some(Emit::Live(_, h_live)) => {
                if tie_to_delete {
                    h_tomb >= *h_live
                } else {
                    h_tomb > *h_live
                }
            }
            Some(Emit::Dead(h_prev)) => h_tomb > *h_prev,
            None => true,
        };
        if replace {
            map.insert(cid, Emit::Dead(h_tomb));
        }
    }

    let mut buf = Vec::new();
    for (cid, e) in map {
        match e {
            Emit::Live(entity, hlc) => {
                serde_json::to_writer(
                    &mut buf,
                    &Live {
                        entity,
                        hlc,
                        deleted: false,
                    },
                )
                .map_err(|e| CrateError::CloudSync(format!("serialize live row: {e}")))?;
            }
            Emit::Dead(hlc) => write_tombstone(&mut buf, bucket, &cid, &hlc)?,
        }
        buf.push(b'\n');
    }
    Ok(buf)
}

fn write_tombstone(buf: &mut Vec<u8>, bucket: &Bucket, cid: &str, hlc: &str) -> Result<()> {
    match bucket.pk_columns() {
        [_] => serde_json::to_writer(
            &mut *buf,
            &EntityTombstone {
                id: cid,
                hlc,
                deleted: true,
            },
        )
        .map_err(|e| CrateError::CloudSync(format!("serialize tombstone: {e}")))?,
        [_, _] => {
            let (a, b) = dirty::split_junction_id(cid)
                .ok_or_else(|| CrateError::CloudSync(format!("malformed junction id {cid:?}")))?;
            let res = match bucket {
                Bucket::PlaylistTracks => serde_json::to_writer(
                    &mut *buf,
                    &PlaylistTrackTombstone {
                        playlist_id: a,
                        track_id: b,
                        hlc,
                        deleted: true,
                    },
                ),
                Bucket::TrackTags => serde_json::to_writer(
                    &mut *buf,
                    &TrackTagTombstone {
                        track_id: a,
                        tag_id: b,
                        hlc,
                        deleted: true,
                    },
                ),
                Bucket::DiscoveryReleaseTags => serde_json::to_writer(
                    &mut *buf,
                    &DiscoveryReleaseTagTombstone {
                        release_id: a,
                        tag_id: b,
                        hlc,
                        deleted: true,
                    },
                ),
                Bucket::PlaylistDiscoveryReleases => serde_json::to_writer(
                    &mut *buf,
                    &PlaylistDiscoveryReleaseTombstone {
                        playlist_id: a,
                        release_id: b,
                        hlc,
                        deleted: true,
                    },
                ),
                _ => {
                    return Err(CrateError::CloudSync(
                        "junction arity on non-junction".into(),
                    ))
                }
            };
            res.map_err(|e| CrateError::CloudSync(format!("serialize tombstone: {e}")))?;
        }
        _ => {
            return Err(CrateError::CloudSync(
                "bucket has unexpected PK arity".into(),
            ))
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// settings serialize (special path: no tombstones, per-key HLC in sync_state)
// ---------------------------------------------------------------------------

fn serialize_settings(conn: &Connection) -> Result<Vec<u8>> {
    let mut stmt = conn.prepare(
        "SELECT s.key, s.value, ss.value \
         FROM settings s \
         JOIN sync_state ss ON ss.key = 'setting_hlc:' || s.key \
         ORDER BY s.key",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut buf = Vec::new();
    for row in rows {
        let (key, value, hlc) = row?;
        if !crate::services::cloud_sync::is_synced_setting(&key) {
            continue;
        }
        serde_json::to_writer(&mut buf, &SettingRow { key, value, hlc })
            .map_err(|e| CrateError::CloudSync(format!("serialize setting: {e}")))?;
        buf.push(b'\n');
    }
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Typed live reads (one per entity/junction bucket). Each returns
// (canonical_id, wire_struct, _hlc), letting `emit` wrap them uniformly.
// ---------------------------------------------------------------------------

fn read_tombstones(conn: &Connection, bucket: &Bucket) -> Result<Vec<(String, String)>> {
    let shard = match bucket {
        Bucket::Tracks(n) => Some(*n),
        _ => None,
    };
    let mut stmt = conn.prepare(
        "SELECT entity_id, _hlc FROM sync_tombstones WHERE entity_type = ?1 ORDER BY entity_id",
    )?;
    let rows = stmt.query_map([bucket.entity_type()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (eid, hlc) = row?;
        if let Some(n) = shard {
            if shard_for_track_id(&eid) != n {
                continue;
            }
        }
        out.push((eid, hlc));
    }
    Ok(out)
}

fn read_live_tracks(
    conn: &Connection,
    bucket: &Bucket,
) -> Result<Vec<(String, BackupTrack, String)>> {
    let shard = match bucket {
        Bucket::Tracks(n) => *n,
        _ => unreachable!(),
    };
    // NOTE: reads the whole tracks table and filters by shard in Rust to match
    // `bucket_for_track_id` semantics exactly. Fine for Phase 1 (test scale).
    let mut stmt = conn.prepare(
        "SELECT id, file_path, file_hash, title, artist, album, year, genre, label, \
         catalog_number, duration_ms, bpm, key, bitrate, sample_rate, format, rating, \
         play_count, date_added, date_modified, last_played, rekordbox_id, artwork_path, \
         artwork_source, color, _hlc FROM tracks",
    )?;
    let rows = stmt.query_map([], |r| {
        let t = BackupTrack {
            id: r.get(0)?,
            file_path: r.get(1)?,
            file_hash: r.get(2)?,
            title: r.get(3)?,
            artist: r.get(4)?,
            album: r.get(5)?,
            year: r.get(6)?,
            genre: r.get(7)?,
            label: r.get(8)?,
            catalog_number: r.get(9)?,
            duration_ms: r.get(10)?,
            bpm: r.get(11)?,
            key: r.get(12)?,
            bitrate: r.get(13)?,
            sample_rate: r.get(14)?,
            format: r.get(15)?,
            rating: r.get(16)?,
            play_count: r.get(17)?,
            date_added: r.get(18)?,
            date_modified: r.get(19)?,
            last_played: r.get(20)?,
            rekordbox_id: r.get(21)?,
            artwork_path: r.get(22)?,
            artwork_source: r.get(23)?,
            color: r.get(24)?,
        };
        let hlc: String = r.get(25)?;
        Ok((t, hlc))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (t, hlc) = row?;
        if shard_for_track_id(&t.id) == shard {
            out.push((t.id.clone(), t, hlc));
        }
    }
    Ok(out)
}

fn read_live_playlists(conn: &Connection) -> Result<Vec<(String, PlaylistRow, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, context, \
         date_created, date_modified, _hlc FROM playlists",
    )?;
    let rows = stmt.query_map([], |r| {
        let p = PlaylistRow {
            id: r.get(0)?,
            name: r.get(1)?,
            parent_id: r.get(2)?,
            is_folder: r.get(3)?,
            is_smart: r.get(4)?,
            smart_rules: r.get(5)?,
            sort_order: r.get(6)?,
            context: r.get(7)?,
            date_created: r.get(8)?,
            date_modified: r.get(9)?,
        };
        let hlc: String = r.get(10)?;
        Ok((p.id.clone(), p, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_live_cues(conn: &Connection) -> Result<Vec<(String, CueRow, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, track_id, position_ms, type, loop_end_ms, hot_cue_index, name, color, _hlc \
         FROM cues",
    )?;
    let rows = stmt.query_map([], |r| {
        let c = CueRow {
            id: r.get(0)?,
            track_id: r.get(1)?,
            position_ms: r.get(2)?,
            cue_type: r.get(3)?,
            loop_end_ms: r.get(4)?,
            hot_cue_index: r.get(5)?,
            name: r.get(6)?,
            color: r.get(7)?,
        };
        let hlc: String = r.get(8)?;
        Ok((c.id.clone(), c, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_live_tag_categories(conn: &Connection) -> Result<Vec<(String, TagCategoryRow, String)>> {
    let mut stmt = conn.prepare("SELECT id, name, color, sort_order, _hlc FROM tag_categories")?;
    let rows = stmt.query_map([], |r| {
        let c = TagCategoryRow {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            sort_order: r.get(3)?,
        };
        let hlc: String = r.get(4)?;
        Ok((c.id.clone(), c, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_live_tags(conn: &Connection) -> Result<Vec<(String, Tag, String)>> {
    let mut stmt =
        conn.prepare("SELECT id, category_id, name, color, sort_order, _hlc FROM tags")?;
    let rows = stmt.query_map([], |r| {
        let t = Tag {
            id: r.get(0)?,
            category_id: r.get(1)?,
            name: r.get(2)?,
            color: r.get(3)?,
            sort_order: r.get(4)?,
        };
        let hlc: String = r.get(5)?;
        Ok((t.id.clone(), t, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_live_track_tags(conn: &Connection) -> Result<Vec<(String, BackupTrackTag, String)>> {
    let mut stmt = conn.prepare("SELECT track_id, tag_id, _hlc FROM track_tags")?;
    let rows = stmt.query_map([], |r| {
        let tt = BackupTrackTag {
            track_id: r.get(0)?,
            tag_id: r.get(1)?,
        };
        let hlc: String = r.get(2)?;
        Ok((tt, hlc))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (tt, hlc) = row?;
        let cid = dirty::junction_entity_id(&tt.track_id, &tt.tag_id);
        out.push((cid, tt, hlc));
    }
    Ok(out)
}

fn read_live_discovery_releases(
    conn: &Connection,
) -> Result<Vec<(String, DiscoveryReleaseRow, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, \
         artwork_path, notes, parent_url, date_added, date_modified, _hlc FROM discovery_releases",
    )?;
    let rows = stmt.query_map([], |r| {
        let d = DiscoveryReleaseRow {
            id: r.get(0)?,
            url: r.get(1)?,
            source_type: r.get(2)?,
            artist: r.get(3)?,
            title: r.get(4)?,
            label: r.get(5)?,
            release_date: r.get(6)?,
            artwork_url: r.get(7)?,
            artwork_path: r.get(8)?,
            notes: r.get(9)?,
            parent_url: r.get(10)?,
            date_added: r.get(11)?,
            date_modified: r.get(12)?,
        };
        let hlc: String = r.get(13)?;
        Ok((d.id.clone(), d, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_live_discovery_tracks(conn: &Connection) -> Result<Vec<(String, DiscoveryTrack, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, release_id, name, position, duration_ms, video_id, is_liked, _hlc \
         FROM discovery_tracks",
    )?;
    let rows = stmt.query_map([], |r| {
        let d = DiscoveryTrack {
            id: r.get(0)?,
            release_id: r.get(1)?,
            name: r.get(2)?,
            position: r.get(3)?,
            duration_ms: r.get(4)?,
            video_id: r.get(5)?,
            is_liked: r.get(6)?,
        };
        let hlc: String = r.get(7)?;
        Ok((d.id.clone(), d, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_live_discovery_release_tags(
    conn: &Connection,
) -> Result<Vec<(String, BackupDiscoveryReleaseTag, String)>> {
    let mut stmt = conn.prepare("SELECT release_id, tag_id, _hlc FROM discovery_release_tags")?;
    let rows = stmt.query_map([], |r| {
        let t = BackupDiscoveryReleaseTag {
            release_id: r.get(0)?,
            tag_id: r.get(1)?,
        };
        let hlc: String = r.get(2)?;
        Ok((t, hlc))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (t, hlc) = row?;
        let cid = dirty::junction_entity_id(&t.release_id, &t.tag_id);
        out.push((cid, t, hlc));
    }
    Ok(out)
}

fn read_live_playlist_discovery_releases(
    conn: &Connection,
) -> Result<Vec<(String, BackupPlaylistDiscoveryRelease, String)>> {
    let mut stmt = conn.prepare(
        "SELECT playlist_id, release_id, position, date_added, _hlc \
         FROM playlist_discovery_releases",
    )?;
    let rows = stmt.query_map([], |r| {
        let pos: Option<i32> = r.get(2)?;
        let date_added: Option<String> = r.get(3)?;
        let p = BackupPlaylistDiscoveryRelease {
            playlist_id: r.get(0)?,
            release_id: r.get(1)?,
            position: pos.unwrap_or(0),
            date_added: date_added.unwrap_or_default(),
        };
        let hlc: String = r.get(4)?;
        Ok((p, hlc))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (p, hlc) = row?;
        let cid = dirty::junction_entity_id(&p.playlist_id, &p.release_id);
        out.push((cid, p, hlc));
    }
    Ok(out)
}

fn read_live_playlist_tracks(conn: &Connection) -> Result<Vec<(String, PlaylistTrack, String)>> {
    let mut stmt = conn
        .prepare("SELECT playlist_id, track_id, position, date_added, _hlc FROM playlist_tracks")?;
    let rows = stmt.query_map([], |r| {
        let p = PlaylistTrack {
            playlist_id: r.get(0)?,
            track_id: r.get(1)?,
            position: r.get(2)?,
            date_added: r.get(3)?,
        };
        let hlc: String = r.get(4)?;
        Ok((p, hlc))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (p, hlc) = row?;
        let cid = dirty::junction_entity_id(&p.playlist_id, &p.track_id);
        out.push((cid, p, hlc));
    }
    Ok(out)
}

fn read_live_library_roots(conn: &Connection) -> Result<Vec<(String, LibraryRootRow, String)>> {
    let mut stmt = conn.prepare("SELECT id, name, _hlc FROM library_roots")?;
    let rows = stmt.query_map([], |r| {
        let l = LibraryRootRow {
            id: r.get(0)?,
            name: r.get(1)?,
        };
        let hlc: String = r.get(2)?;
        Ok((l.id.clone(), l, hlc))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_hash_is_deterministic() {
        assert_eq!(bucket_hash(b"abc"), bucket_hash(b"abc"));
        assert_ne!(bucket_hash(b"abc"), bucket_hash(b"abd"));
        // An empty bucket hashes to a stable fixed value (INCLUDE-ALL relies on it).
        assert_eq!(bucket_hash(b""), bucket_hash(b""));
    }

    #[test]
    fn live_envelope_field_order_is_declaration_order() {
        let row = Live {
            entity: LibraryRootRow {
                id: "r1".into(),
                name: "Music".into(),
            },
            hlc: "00ff".into(),
            deleted: false,
        };
        let s = serde_json::to_string(&row).unwrap();
        assert_eq!(
            s,
            r#"{"id":"r1","name":"Music","_hlc":"00ff","_deleted":false}"#
        );
    }

    #[test]
    fn parse_extracts_meta_and_validates_pk() {
        let bytes = b"{\"id\":\"x\",\"name\":\"n\",\"_hlc\":\"00\",\"_deleted\":false}\n\
                      {\"id\":\"y\",\"_hlc\":\"01\",\"_deleted\":true}\n";
        let rows = parse_bucket(&Bucket::LibraryRoots, bytes).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].hlc, "00");
        assert!(!rows[0].deleted);
        assert_eq!(rows[1].hlc, "01");
        assert!(rows[1].deleted);
    }

    #[test]
    fn parse_rejects_row_missing_pk() {
        let bytes = b"{\"name\":\"n\",\"_hlc\":\"00\"}\n";
        assert!(parse_bucket(&Bucket::LibraryRoots, bytes).is_err());
    }

    #[test]
    fn canonical_id_entity_and_junction() {
        let v = serde_json::json!({"id": "abc"});
        assert_eq!(canonical_id(&Bucket::Playlists, &v).unwrap(), "abc");
        let v = serde_json::json!({"track_id": "t1", "tag_id": "g1"});
        assert_eq!(
            canonical_id(&Bucket::TrackTags, &v).unwrap(),
            dirty::junction_entity_id("t1", "g1")
        );
    }

    #[test]
    fn cue_row_wire_key_is_cue_type() {
        let c = CueRow {
            id: "c".into(),
            track_id: "t".into(),
            position_ms: 0,
            cue_type: "hot".into(),
            loop_end_ms: None,
            hot_cue_index: None,
            name: None,
            color: None,
        };
        let s = serde_json::to_string(&c).unwrap();
        assert!(s.contains(r#""cue_type":"hot""#), "got {s}");
    }
}
