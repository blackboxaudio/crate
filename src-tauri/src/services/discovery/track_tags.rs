//! Track-level discovery tags (`discovery_track_tags`), which coexist with the
//! release-level `discovery_release_tags`: some tags describe a whole release
//! (label, style), others a single track ("the one for the mix").

use rusqlite::Connection;

use super::*;

/// Tables keyed by a discovery track id. Every path that deletes a losing track
/// row in favour of a surviving duplicate (dedupe sweep, sync collapse, release
/// merge) must re-point these to the survivor first, or the membership/tag is
/// silently lost with the row.
pub const TRACK_JUNCTIONS: &[&str] = &["playlist_discovery_tracks", "discovery_track_tags"];

/// Move every track-keyed junction row from `loser` to `survivor`, dropping the
/// leftovers whose `(survivor, other)` pair already exists. Returns whether any row
/// moved or was dropped, so callers can mark the buckets dirty only when needed.
pub fn repoint_track_junctions(conn: &Connection, loser: &str, survivor: &str) -> Result<bool> {
    let mut touched = false;
    for table in TRACK_JUNCTIONS {
        let moved = conn.execute(
            &format!("UPDATE OR IGNORE {table} SET track_id = ?1 WHERE track_id = ?2"),
            rusqlite::params![survivor, loser],
        )?;
        let dropped = conn.execute(&format!("DELETE FROM {table} WHERE track_id = ?1"), [loser])?;
        touched = touched || moved > 0 || dropped > 0;
    }
    Ok(touched)
}

/// Fill `tags` on every track from `discovery_track_tags`, in one batched query per
/// 500 ids. Shared by every `DiscoveryRelease` loader so the nested DTO shape is the
/// same whichever path produced it.
pub fn attach_track_tags(conn: &Connection, tracks: &mut [DiscoveryTrack]) -> Result<()> {
    if tracks.is_empty() {
        return Ok(());
    }
    let mut by_track: std::collections::HashMap<String, Vec<Tag>> =
        std::collections::HashMap::new();
    let ids: Vec<&str> = tracks.iter().map(|t| t.id.as_str()).collect();
    for chunk in ids.chunks(500) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let mut stmt = conn.prepare(&format!(
            "SELECT dtt.track_id, t.id, t.category_id, t.name, t.color, t.sort_order
             FROM tags t
             INNER JOIN discovery_track_tags dtt ON t.id = dtt.tag_id
             WHERE dtt.track_id IN ({placeholders})
             ORDER BY t.sort_order, t.name"
        ))?;
        let params: Vec<&dyn rusqlite::ToSql> =
            chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                Tag {
                    id: row.get(1)?,
                    category_id: row.get(2)?,
                    name: row.get(3)?,
                    color: row.get(4)?,
                    sort_order: row.get(5)?,
                },
            ))
        })?;
        for row in rows {
            let (track_id, tag) = row?;
            by_track.entry(track_id).or_default().push(tag);
        }
    }
    if by_track.is_empty() {
        return Ok(());
    }
    for track in tracks.iter_mut() {
        if let Some(tags) = by_track.remove(&track.id) {
            track.tags = tags;
        }
    }
    Ok(())
}

impl DiscoveryService {
    pub fn assign_track_tags(&self, track_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for track_id in &track_ids {
            for tag_id in &tag_ids {
                conn.execute(
                    "INSERT OR IGNORE INTO discovery_track_tags (track_id, tag_id, _hlc) VALUES (?1, ?2, ?3)",
                    rusqlite::params![track_id, tag_id, hlc],
                )?;
            }
        }
        dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACK_TAGS)?;

        Ok(())
    }

    pub fn remove_track_tags(&self, track_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for track_id in &track_ids {
            for tag_id in &tag_ids {
                let deleted = conn.execute(
                    "DELETE FROM discovery_track_tags WHERE track_id = ?1 AND tag_id = ?2",
                    rusqlite::params![track_id, tag_id],
                )?;
                if deleted > 0 {
                    dirty::record_tombstone(
                        &conn,
                        buckets::DISCOVERY_TRACK_TAGS,
                        &dirty::junction_entity_id(track_id, tag_id),
                        &hlc,
                    )?;
                }
            }
        }
        dirty::mark_dirty(&conn, buckets::DISCOVERY_TRACK_TAGS)?;

        Ok(())
    }
}
