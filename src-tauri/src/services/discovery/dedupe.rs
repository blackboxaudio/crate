//! One-shot heal for historically duplicated discovery tracks.
//!
//! Track ids used to be random v4 UUIDs with no natural-key uniqueness, so two
//! devices independently fetching the same release's tracks (e.g. both refreshing
//! a release that synced without tracks) produced duplicate rows that cloud sync
//! unioned everywhere. New rows mint content-derived ids and the sync merge now
//! collapses cross-device duplicates; this sweep collapses the ones already
//! sitting in the local database.
//!
//! Runs on every launch: it is one scan of a small table, a no-op without
//! duplicate groups, and re-running also heals duplicates introduced later (a
//! restored backup, a peer on an older build pushing while this device was off).
//!
//! Collapse semantics mirror the merge engine's `collapse` module: keep the
//! smallest id of each `(release_id, normalized name)` group, fold fields
//! winner-first (highest `_hlc` wins, `NULL`s coalesced), OR the `is_liked`
//! flags, and tombstone the removed ids so peers (and the cloud union) drop them
//! too.
//!
//! Every stamp this sweep writes is `hlc::bump` of a stamp already in the group —
//! NEVER a fresh clock reading. Minimal dominance is load-bearing twice over:
//! a fresh stamp would outrank (and destroy) a peer's concurrent-but-unsynced
//! edit to one of these rows — e.g. a like on the duplicate copy — whereas a
//! bumped stamp loses to it, keeps the edit alive, and the merge collapse folds
//! it into the survivor on the next sync; and being a pure function of the rows,
//! two devices sweeping the same converged state write byte-identical results.

use rusqlite::Connection;

use crate::error::Result;
use crate::models::{normalized_track_name, DiscoveryTrack};
use crate::services::cloud_sync::hlc;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

/// Collapse duplicate discovery tracks sharing `(release_id, normalized name)`.
/// Returns the number of rows removed. Idempotent; safe to run every launch.
pub fn dedupe_discovery_tracks(conn: &Connection) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, release_id, name, position, duration_ms, video_id, url, is_liked, _hlc \
         FROM discovery_tracks ORDER BY release_id, id",
    )?;
    let rows: Vec<(DiscoveryTrack, String)> = stmt
        .query_map([], |r| {
            Ok((
                DiscoveryTrack {
                    id: r.get(0)?,
                    release_id: r.get(1)?,
                    name: r.get(2)?,
                    position: r.get(3)?,
                    duration_ms: r.get(4)?,
                    video_id: r.get(5)?,
                    url: r.get(6)?,
                    is_liked: r.get::<_, i32>(7).map(|v| v != 0)?,
                },
                r.get::<_, String>(8)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut groups: std::collections::BTreeMap<(String, String), Vec<(DiscoveryTrack, String)>> =
        std::collections::BTreeMap::new();
    for (track, track_hlc) in rows {
        groups
            .entry((track.release_id.clone(), normalized_track_name(&track.name)))
            .or_default()
            .push((track, track_hlc));
    }
    groups.retain(|_, members| members.len() > 1);
    if groups.is_empty() {
        return Ok(0);
    }

    let tx = conn.unchecked_transaction()?;
    let mut removed = 0usize;

    for (_, mut members) in groups {
        // Winner-first: highest `_hlc`, ties to the smaller id (same rule as the merge
        // collapse). The smallest id in the group survives regardless of who wins —
        // "keep smallest id" is what makes independent sweeps on two devices commute.
        members.sort_by(|(a, a_hlc), (b, b_hlc)| {
            b_hlc.cmp(a_hlc).then_with(|| a.id.cmp(&b.id))
        });
        let group_max_hlc = members[0].1.clone();
        let survivor_id = members
            .iter()
            .map(|(t, _)| t.id.clone())
            .min()
            .expect("non-empty group");

        let mut merged = members[0].0.clone();
        for (t, _) in members.iter().skip(1) {
            merged.duration_ms = merged.duration_ms.or(t.duration_ms);
            merged.video_id = merged.video_id.clone().or_else(|| t.video_id.clone());
            merged.url = merged.url.clone().or_else(|| t.url.clone());
            merged.is_liked = merged.is_liked || t.is_liked;
        }
        merged.id = survivor_id.clone();

        let survivor_row = members
            .iter()
            .find(|(t, _)| t.id == survivor_id)
            .expect("survivor is a member")
            .0
            .clone();

        // Re-stamp only when the survivor's content actually changes; the stamp outranks
        // every copy in this group (so the merged content wins LWW against them on peers)
        // but nothing newer.
        if merged != survivor_row {
            let stamp = hlc::bump(&group_max_hlc);
            tx.execute(
                "UPDATE discovery_tracks SET name = ?1, position = ?2, duration_ms = ?3, \
                 video_id = ?4, url = ?5, is_liked = ?6, _hlc = ?7 WHERE id = ?8",
                rusqlite::params![
                    merged.name,
                    merged.position,
                    merged.duration_ms,
                    merged.video_id,
                    merged.url,
                    merged.is_liked,
                    stamp,
                    survivor_id,
                ],
            )?;
        }

        for (t, t_hlc) in members.iter().filter(|(t, _)| t.id != survivor_id) {
            tx.execute("DELETE FROM discovery_tracks WHERE id = ?1", [&t.id])?;
            // Strictly above THIS copy (kills identical copies on peers), but minimally
            // so — a peer's later edit to its copy outranks the tombstone, survives, and
            // is folded into the survivor by the merge collapse.
            dirty::record_tombstone(&tx, buckets::DISCOVERY_TRACKS, &t.id, &hlc::bump(t_hlc))?;
            removed += 1;
        }
    }

    dirty::mark_dirty(&tx, buckets::DISCOVERY_TRACKS)?;
    tx.commit()?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cloud_sync::hlc::Hlc;
    use crate::services::cloud_sync::pipeline::buckets::Bucket;
    use crate::services::cloud_sync::pipeline::rows;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        for sql in crate::db::schema::get_migrations() {
            conn.execute_batch(sql).unwrap();
        }
        conn
    }

    fn h(wall: u64) -> String {
        Hlc::new(wall, 0, 0xA).format()
    }

    fn seed_release(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, date_added, date_modified, _hlc) \
             VALUES (?1, ?2, 'bandcamp', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', ?3)",
            rusqlite::params![id, format!("https://x.bandcamp.com/{id}"), h(1)],
        )
        .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_track(
        conn: &Connection,
        id: &str,
        release: &str,
        name: &str,
        wall: u64,
        liked: bool,
        duration_ms: Option<i64>,
    ) {
        conn.execute(
            "INSERT INTO discovery_tracks (id, release_id, name, position, duration_ms, is_liked, _hlc) \
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
            rusqlite::params![id, release, name, duration_ms, liked, h(wall)],
        )
        .unwrap();
    }

    fn track_rows(conn: &Connection, release: &str) -> Vec<(String, bool, Option<i64>, String)> {
        let mut stmt = conn
            .prepare(
                "SELECT id, is_liked, duration_ms, _hlc FROM discovery_tracks \
                 WHERE release_id = ?1 ORDER BY id",
            )
            .unwrap();
        stmt.query_map([release], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i32>(1)? != 0,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
    }

    #[test]
    fn clean_db_is_a_noop() {
        let conn = mem_db();
        seed_release(&conn, "rel-1");
        seed_track(&conn, "aaaa", "rel-1", "Intro", 10, false, None);
        seed_track(&conn, "bbbb", "rel-1", "Outro", 11, false, None);
        assert_eq!(dedupe_discovery_tracks(&conn).unwrap(), 0);
        assert_eq!(track_rows(&conn, "rel-1").len(), 2);
    }

    #[test]
    fn three_way_group_keeps_smallest_id_ors_likes_coalesces_fields() {
        let conn = mem_db();
        seed_release(&conn, "rel-1");
        seed_track(&conn, "cccc", "rel-1", "Intro", 30, false, Some(1000));
        seed_track(&conn, "aaaa", "rel-1", "intro", 10, false, None);
        seed_track(&conn, "bbbb", "rel-1", " INTRO ", 20, true, None);

        assert_eq!(dedupe_discovery_tracks(&conn).unwrap(), 2);
        let rows = track_rows(&conn, "rel-1");
        assert_eq!(rows.len(), 1);
        let (id, liked, duration, hlc) = &rows[0];
        assert_eq!(id, "aaaa", "smallest id survives");
        assert!(*liked, "like OR-ed from a removed copy");
        assert_eq!(*duration, Some(1000), "duration coalesced from the winner");
        assert!(*hlc > h(30), "changed survivor re-stamped above the group max");

        // Tombstones strictly outrank the removed copies, so identical copies on
        // peers are deleted — but only minimally, so a newer edit there survives.
        for (loser, wall) in [("bbbb", 20u64), ("cccc", 30u64)] {
            let tomb: String = conn
                .query_row(
                    "SELECT _hlc FROM sync_tombstones WHERE entity_type = 'discovery_tracks' \
                     AND entity_id = ?1",
                    [loser],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(tomb > h(wall));
            assert!(tomb < h(wall + 1), "minimal dominance: below the next wall tick");
        }
    }

    #[test]
    fn second_run_changes_nothing() {
        let conn = mem_db();
        seed_release(&conn, "rel-1");
        seed_track(&conn, "aaaa", "rel-1", "Intro", 10, false, None);
        seed_track(&conn, "bbbb", "rel-1", "Intro", 20, true, None);

        assert_eq!(dedupe_discovery_tracks(&conn).unwrap(), 1);
        let bytes = rows::serialize_bucket(&conn, &Bucket::DiscoveryTracks).unwrap();
        assert_eq!(dedupe_discovery_tracks(&conn).unwrap(), 0);
        assert_eq!(
            rows::serialize_bucket(&conn, &Bucket::DiscoveryTracks).unwrap(),
            bytes,
            "idempotent: re-running produces identical bytes"
        );
    }

    #[test]
    fn groups_are_scoped_per_release() {
        let conn = mem_db();
        seed_release(&conn, "rel-1");
        seed_release(&conn, "rel-2");
        seed_track(&conn, "aaaa", "rel-1", "Intro", 10, false, None);
        seed_track(&conn, "bbbb", "rel-2", "Intro", 20, false, None);
        assert_eq!(
            dedupe_discovery_tracks(&conn).unwrap(),
            0,
            "same name on different releases is not a duplicate"
        );
    }
}
