//! The SQL side of the merge engine: per-entity UPSERTs, junction insert/update/
//! delete, endpoint guards, and tombstone bookkeeping.
//!
//! Two invariants live here:
//! - **UPSERTs write ONLY synced columns.** The track UPSERT never touches
//!   `analysis_source`, `waveform_data`, `library_root_id`, or `relative_path`, so a
//!   peer that analyzed a track keeps its waveform when it pulls a metadata edit.
//! - **Entity upserts run inside a per-row SAVEPOINT** and skip (rather than abort
//!   the whole bucket) on a secondary `UNIQUE` collision — two devices that
//!   independently created e.g. a tag category named "House" must not wedge sync.

use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;

use crate::error::{CrateError, Result};
use crate::models::{
    BackupDiscoveryReleaseTag, BackupPlaylistDiscoveryRelease, BackupTrack, BackupTrackTag,
    DiscoveryTrack, PlaylistTrack, Tag,
};

use super::super::buckets::Bucket;
use super::super::dirty;
use super::super::rows::{
    CueRow, DiscoveryReleaseRow, DiscoveryReleaseSourceRow, FollowedSourceRow, LibraryRootRow,
    ParsedRow, PlaylistRow, TagCategoryRow,
};

// SQLite extended result codes for the constraint violations we tolerate.
const SQLITE_CONSTRAINT_UNIQUE: i32 = 2067;
const SQLITE_CONSTRAINT_PRIMARYKEY: i32 = 1555;

fn de<T: DeserializeOwned>(value: &serde_json::Value) -> Result<T> {
    serde_json::from_value(value.clone())
        .map_err(|e| CrateError::CloudSync(format!("deserialize sync row: {e}")))
}

/// True for a `UNIQUE`/`PRIMARY KEY` collision — the only constraint failures a
/// merge tolerates (the row is skipped, the rest of the bucket still merges).
fn is_skippable_constraint(e: &CrateError) -> bool {
    matches!(
        e,
        CrateError::Database(rusqlite::Error::SqliteFailure(f, _))
            if f.extended_code == SQLITE_CONSTRAINT_UNIQUE
                || f.extended_code == SQLITE_CONSTRAINT_PRIMARYKEY
    )
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// UPSERT a live entity row, isolated in a SAVEPOINT so a secondary UNIQUE
/// collision (e.g. duplicate `tag_categories.name`) skips this row instead of
/// aborting the bucket transaction.
pub(super) fn upsert_entity(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<()> {
    tx.execute_batch("SAVEPOINT merge_row")?;
    match upsert_entity_inner(tx, bucket, row) {
        Ok(()) => {
            tx.execute_batch("RELEASE merge_row")?;
            Ok(())
        }
        Err(e) => {
            let _ = tx.execute_batch("ROLLBACK TO merge_row");
            let _ = tx.execute_batch("RELEASE merge_row");
            if is_skippable_constraint(&e) {
                log::warn!(
                    "cloud_sync merge: skipped a {} row on a UNIQUE collision: {e}",
                    bucket.as_str()
                );
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

fn upsert_entity_inner(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<()> {
    let v = &row.value;
    let hlc = &row.hlc;
    match bucket {
        Bucket::Tracks(_) => upsert_track(tx, &de(v)?, hlc),
        Bucket::Playlists => upsert_playlist(tx, &de(v)?, hlc),
        Bucket::Cues => upsert_cue(tx, &de(v)?, hlc),
        Bucket::TagCategories => upsert_tag_category(tx, &de(v)?, hlc),
        Bucket::Tags => upsert_tag(tx, &de(v)?, hlc),
        Bucket::DiscoveryReleases => upsert_discovery_release(tx, &de(v)?, hlc),
        Bucket::DiscoveryTracks => upsert_discovery_track(tx, &de(v)?, hlc),
        Bucket::FollowedSources => upsert_followed_source(tx, &de(v)?, hlc),
        Bucket::LibraryRoots => upsert_library_root(tx, &de(v)?, hlc),
        _ => Err(CrateError::CloudSync(format!(
            "upsert_entity on non-entity bucket {}",
            bucket.as_str()
        ))),
    }
}

fn upsert_track(tx: &Connection, t: &BackupTrack, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO tracks \
            (id, file_path, file_hash, title, artist, album, year, genre, label, catalog_number, \
             duration_ms, bpm, key, bitrate, sample_rate, format, rating, play_count, date_added, \
             date_modified, last_played, rekordbox_id, artwork_path, artwork_source, color, _hlc) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26) \
         ON CONFLICT(id) DO UPDATE SET \
            file_path=excluded.file_path, file_hash=excluded.file_hash, title=excluded.title, \
            artist=excluded.artist, album=excluded.album, year=excluded.year, genre=excluded.genre, \
            label=excluded.label, catalog_number=excluded.catalog_number, duration_ms=excluded.duration_ms, \
            bpm=excluded.bpm, key=excluded.key, bitrate=excluded.bitrate, sample_rate=excluded.sample_rate, \
            format=excluded.format, rating=excluded.rating, play_count=excluded.play_count, \
            date_added=excluded.date_added, date_modified=excluded.date_modified, \
            last_played=excluded.last_played, rekordbox_id=excluded.rekordbox_id, \
            artwork_path=excluded.artwork_path, artwork_source=excluded.artwork_source, \
            color=excluded.color, _hlc=excluded._hlc",
        params![
            t.id, t.file_path, t.file_hash, t.title, t.artist, t.album, t.year, t.genre, t.label,
            t.catalog_number, t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
            t.rating, t.play_count, t.date_added, t.date_modified, t.last_played, t.rekordbox_id,
            t.artwork_path, t.artwork_source, t.color, hlc,
        ],
    )?;
    Ok(())
}

fn upsert_playlist(tx: &Connection, p: &PlaylistRow, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO playlists \
            (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, context, \
             date_created, date_modified, _hlc) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) \
         ON CONFLICT(id) DO UPDATE SET \
            name=excluded.name, parent_id=excluded.parent_id, is_folder=excluded.is_folder, \
            is_smart=excluded.is_smart, smart_rules=excluded.smart_rules, sort_order=excluded.sort_order, \
            context=excluded.context, date_created=excluded.date_created, \
            date_modified=excluded.date_modified, _hlc=excluded._hlc",
        params![
            p.id, p.name, p.parent_id, p.is_folder, p.is_smart, p.smart_rules, p.sort_order,
            p.context, p.date_created, p.date_modified, hlc,
        ],
    )?;
    Ok(())
}

fn upsert_cue(tx: &Connection, c: &CueRow, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO cues \
            (id, track_id, position_ms, type, loop_end_ms, hot_cue_index, name, color, _hlc) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) \
         ON CONFLICT(id) DO UPDATE SET \
            track_id=excluded.track_id, position_ms=excluded.position_ms, type=excluded.type, \
            loop_end_ms=excluded.loop_end_ms, hot_cue_index=excluded.hot_cue_index, \
            name=excluded.name, color=excluded.color, _hlc=excluded._hlc",
        params![
            c.id,
            c.track_id,
            c.position_ms,
            c.cue_type,
            c.loop_end_ms,
            c.hot_cue_index,
            c.name,
            c.color,
            hlc,
        ],
    )?;
    Ok(())
}

fn upsert_tag_category(tx: &Connection, c: &TagCategoryRow, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO tag_categories (id, name, color, sort_order, _hlc) VALUES (?1,?2,?3,?4,?5) \
         ON CONFLICT(id) DO UPDATE SET \
            name=excluded.name, color=excluded.color, sort_order=excluded.sort_order, _hlc=excluded._hlc",
        params![c.id, c.name, c.color, c.sort_order, hlc],
    )?;
    Ok(())
}

fn upsert_tag(tx: &Connection, t: &Tag, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO tags (id, category_id, name, color, sort_order, _hlc) VALUES (?1,?2,?3,?4,?5,?6) \
         ON CONFLICT(id) DO UPDATE SET \
            category_id=excluded.category_id, name=excluded.name, color=excluded.color, \
            sort_order=excluded.sort_order, _hlc=excluded._hlc",
        params![t.id, t.category_id, t.name, t.color, t.sort_order, hlc],
    )?;
    Ok(())
}

fn upsert_discovery_release(tx: &Connection, d: &DiscoveryReleaseRow, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO discovery_releases \
            (id, url, source_type, artist, title, label, release_date, artwork_url, artwork_path, \
             notes, parent_url, date_added, date_modified, is_new, surfaced_at, _hlc) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) \
         ON CONFLICT(id) DO UPDATE SET \
            url=excluded.url, source_type=excluded.source_type, artist=excluded.artist, \
            title=excluded.title, label=excluded.label, release_date=excluded.release_date, \
            artwork_url=excluded.artwork_url, artwork_path=excluded.artwork_path, notes=excluded.notes, \
            parent_url=excluded.parent_url, date_added=excluded.date_added, \
            date_modified=excluded.date_modified, is_new=excluded.is_new, \
            surfaced_at=excluded.surfaced_at, _hlc=excluded._hlc",
        params![
            d.id, d.url, d.source_type, d.artist, d.title, d.label, d.release_date, d.artwork_url,
            d.artwork_path, d.notes, d.parent_url, d.date_added, d.date_modified, d.is_new,
            d.surfaced_at, hlc,
        ],
    )?;
    Ok(())
}

fn upsert_discovery_track(tx: &Connection, d: &DiscoveryTrack, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO discovery_tracks \
            (id, release_id, name, position, duration_ms, video_id, is_liked, _hlc) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
         ON CONFLICT(id) DO UPDATE SET \
            release_id=excluded.release_id, name=excluded.name, position=excluded.position, \
            duration_ms=excluded.duration_ms, video_id=excluded.video_id, is_liked=excluded.is_liked, \
            _hlc=excluded._hlc",
        params![
            d.id, d.release_id, d.name, d.position, d.duration_ms, d.video_id, d.is_liked, hlc,
        ],
    )?;
    Ok(())
}

fn upsert_library_root(tx: &Connection, l: &LibraryRootRow, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO library_roots (id, name, _hlc) VALUES (?1,?2,?3) \
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, _hlc=excluded._hlc",
        params![l.id, l.name, hlc],
    )?;
    Ok(())
}

fn upsert_followed_source(tx: &Connection, f: &FollowedSourceRow, hlc: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO followed_sources \
            (id, url, source_type, follow_type, name, artwork_url, artwork_path, enabled, \
             date_added, date_modified, _hlc) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) \
         ON CONFLICT(id) DO UPDATE SET \
            url=excluded.url, source_type=excluded.source_type, follow_type=excluded.follow_type, \
            name=excluded.name, artwork_url=excluded.artwork_url, artwork_path=excluded.artwork_path, \
            enabled=excluded.enabled, date_added=excluded.date_added, \
            date_modified=excluded.date_modified, _hlc=excluded._hlc",
        params![
            f.id, f.url, f.source_type, f.follow_type, f.name, f.artwork_url, f.artwork_path,
            f.enabled, f.date_added, f.date_modified, hlc,
        ],
    )?;
    // A source synced in from another device has no local watch state yet. Seed a
    // default row (baseline_established=0) so the watch loop establishes a baseline on
    // first sight and does NOT flood Discovery with the back catalog. OR IGNORE leaves
    // any existing local state untouched.
    tx.execute(
        "INSERT OR IGNORE INTO followed_source_state (source_id) VALUES (?1)",
        params![f.id],
    )?;
    Ok(())
}

/// Hard-delete an entity by its single PK. FK `ON DELETE CASCADE` removes its
/// children/junction rows (mirroring what the deleting device did).
pub(super) fn hard_delete_entity(tx: &Connection, bucket: &Bucket, cid: &str) -> Result<()> {
    let table = bucket.table();
    tx.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [cid])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Junctions
// ---------------------------------------------------------------------------

pub(super) fn insert_junction(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<()> {
    let v = &row.value;
    let hlc = &row.hlc;
    match bucket {
        Bucket::PlaylistTracks => {
            let p: PlaylistTrack = de(v)?;
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position, date_added, _hlc) \
                 VALUES (?1,?2,?3,?4,?5) \
                 ON CONFLICT(playlist_id, track_id) DO UPDATE SET \
                    position=excluded.position, date_added=excluded.date_added, _hlc=excluded._hlc",
                params![p.playlist_id, p.track_id, p.position, p.date_added, hlc],
            )?;
        }
        Bucket::PlaylistDiscoveryReleases => {
            let p: BackupPlaylistDiscoveryRelease = de(v)?;
            tx.execute(
                "INSERT INTO playlist_discovery_releases (playlist_id, release_id, position, date_added, _hlc) \
                 VALUES (?1,?2,?3,?4,?5) \
                 ON CONFLICT(playlist_id, release_id) DO UPDATE SET \
                    position=excluded.position, date_added=excluded.date_added, _hlc=excluded._hlc",
                params![p.playlist_id, p.release_id, p.position, p.date_added, hlc],
            )?;
        }
        Bucket::TrackTags => {
            let t: BackupTrackTag = de(v)?;
            tx.execute(
                "INSERT INTO track_tags (track_id, tag_id, _hlc) VALUES (?1,?2,?3) \
                 ON CONFLICT(track_id, tag_id) DO UPDATE SET _hlc=excluded._hlc",
                params![t.track_id, t.tag_id, hlc],
            )?;
        }
        Bucket::DiscoveryReleaseTags => {
            let t: BackupDiscoveryReleaseTag = de(v)?;
            tx.execute(
                "INSERT INTO discovery_release_tags (release_id, tag_id, _hlc) VALUES (?1,?2,?3) \
                 ON CONFLICT(release_id, tag_id) DO UPDATE SET _hlc=excluded._hlc",
                params![t.release_id, t.tag_id, hlc],
            )?;
        }
        Bucket::DiscoveryReleaseSources => {
            let s: DiscoveryReleaseSourceRow = de(v)?;
            tx.execute(
                "INSERT INTO discovery_release_sources (release_id, source_id, _hlc) VALUES (?1,?2,?3) \
                 ON CONFLICT(release_id, source_id) DO UPDATE SET _hlc=excluded._hlc",
                params![s.release_id, s.source_id, hlc],
            )?;
        }
        _ => {
            return Err(CrateError::CloudSync(format!(
                "insert_junction on non-junction bucket {}",
                bucket.as_str()
            )))
        }
    }
    Ok(())
}

/// Overwrite the ordering fields (position, date_added) + `_hlc` of an existing
/// ordered-junction row (LWW). Only `playlist_tracks` / `playlist_discovery_releases`.
pub(super) fn upsert_junction_ordering(
    tx: &Connection,
    bucket: &Bucket,
    row: &ParsedRow,
) -> Result<()> {
    let v = &row.value;
    let hlc = &row.hlc;
    match bucket {
        Bucket::PlaylistTracks => {
            let p: PlaylistTrack = de(v)?;
            tx.execute(
                "UPDATE playlist_tracks SET position=?1, date_added=?2, _hlc=?3 \
                 WHERE playlist_id=?4 AND track_id=?5",
                params![p.position, p.date_added, hlc, p.playlist_id, p.track_id],
            )?;
        }
        Bucket::PlaylistDiscoveryReleases => {
            let p: BackupPlaylistDiscoveryRelease = de(v)?;
            tx.execute(
                "UPDATE playlist_discovery_releases SET position=?1, date_added=?2, _hlc=?3 \
                 WHERE playlist_id=?4 AND release_id=?5",
                params![p.position, p.date_added, hlc, p.playlist_id, p.release_id],
            )?;
        }
        _ => {
            return Err(CrateError::CloudSync(format!(
                "upsert_junction_ordering on non-ordered bucket {}",
                bucket.as_str()
            )))
        }
    }
    Ok(())
}

/// Advance an existing junction row's `_hlc` (used for tag junctions, which have
/// no ordering fields — only the clock needs to converge).
pub(super) fn advance_junction_hlc(
    tx: &Connection,
    bucket: &Bucket,
    cid: &str,
    hlc: &str,
) -> Result<()> {
    let (a, b) = split_pk(cid)?;
    let cols = bucket.pk_columns();
    let table = bucket.table();
    tx.execute(
        &format!(
            "UPDATE {table} SET _hlc=?1 WHERE {}=?2 AND {}=?3",
            cols[0], cols[1]
        ),
        params![hlc, a, b],
    )?;
    Ok(())
}

pub(super) fn delete_junction(tx: &Connection, bucket: &Bucket, cid: &str) -> Result<()> {
    let (a, b) = split_pk(cid)?;
    let cols = bucket.pk_columns();
    let table = bucket.table();
    tx.execute(
        &format!(
            "DELETE FROM {table} WHERE {}=?1 AND {}=?2",
            cols[0], cols[1]
        ),
        params![a, b],
    )?;
    Ok(())
}

/// Whether both of a junction row's endpoints exist locally. A junction is only
/// inserted when this holds, so a concurrent re-add over a cascade-deleted parent
/// is skipped instead of violating an FK at commit.
pub(super) fn junction_endpoints_exist(
    tx: &Connection,
    bucket: &Bucket,
    row: &ParsedRow,
) -> Result<bool> {
    let (p0_table, p0_col, p1_table, p1_col) = match bucket {
        Bucket::PlaylistTracks => ("playlists", "playlist_id", "tracks", "track_id"),
        Bucket::TrackTags => ("tracks", "track_id", "tags", "tag_id"),
        Bucket::DiscoveryReleaseTags => ("discovery_releases", "release_id", "tags", "tag_id"),
        Bucket::PlaylistDiscoveryReleases => (
            "playlists",
            "playlist_id",
            "discovery_releases",
            "release_id",
        ),
        Bucket::DiscoveryReleaseSources => (
            "discovery_releases",
            "release_id",
            "followed_sources",
            "source_id",
        ),
        _ => return Ok(true),
    };
    let v = &row.value;
    let id0 = v
        .get(p0_col)
        .and_then(|x| x.as_str())
        .ok_or_else(|| CrateError::CloudSync(format!("junction row missing {p0_col}")))?;
    let id1 = v
        .get(p1_col)
        .and_then(|x| x.as_str())
        .ok_or_else(|| CrateError::CloudSync(format!("junction row missing {p1_col}")))?;
    let e0: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {p0_table} WHERE id=?1)"),
        [id0],
        |r| r.get(0),
    )?;
    let e1: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {p1_table} WHERE id=?1)"),
        [id1],
        |r| r.get(0),
    )?;
    Ok(e0 && e1)
}

// ---------------------------------------------------------------------------
// Tombstones
// ---------------------------------------------------------------------------

/// Record or advance a tombstone, keeping the lexicographically-larger `_hlc`
/// (`MAX`), so a caller can never accidentally lower an existing tombstone.
pub(super) fn upsert_tombstone(
    tx: &Connection,
    bucket: &Bucket,
    cid: &str,
    hlc: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO sync_tombstones (entity_type, entity_id, _hlc) VALUES (?1, ?2, ?3) \
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET _hlc = MAX(_hlc, excluded._hlc)",
        params![bucket.entity_type(), cid, hlc],
    )?;
    Ok(())
}

pub(super) fn delete_tombstone(tx: &Connection, bucket: &Bucket, cid: &str) -> Result<()> {
    tx.execute(
        "DELETE FROM sync_tombstones WHERE entity_type=?1 AND entity_id=?2",
        params![bucket.entity_type(), cid],
    )?;
    Ok(())
}

fn split_pk(cid: &str) -> Result<(&str, &str)> {
    dirty::split_junction_id(cid)
        .ok_or_else(|| CrateError::CloudSync(format!("malformed junction id {cid:?}")))
}
