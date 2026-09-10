//! Fan whole-release playlist memberships out into per-track rows.
//!
//! Discovery playlists record membership per track (`playlist_discovery_tracks`).
//! The older `playlist_discovery_releases` junction is kept as a ledger of
//! "whole release, pending expansion": rows land there when a release is added
//! before its tracks have been fetched (follow-watch and bulk import create
//! trackless releases and enrich them later), when a peer on an older build adds
//! a release, or when a pre-transition backup is restored. As soon as the
//! release has tracks, this sweep replaces the ledger row with one row per track
//! — order preserved, appended after whatever the playlist already holds — and
//! tombstones the ledger row so peers drop it too.
//!
//! Every stamp written here is `hlc::bump` of the ledger row's own stamp, never a
//! fresh clock reading (the same minimal-dominance rule as the dedupe and
//! renormalize sweeps): a bumped stamp outranks the ledger row it replaces on
//! every peer but loses to any later user edit, and being a pure function of
//! the row, two devices expanding the same converged ledger write identical
//! bytes. A ledger row that was never stamped (`_hlc = ''`, i.e. never pushed)
//! produces unstamped track rows and no tombstone — the first push's one-time
//! backfill stamps them from `date_added` exactly as it would have stamped the
//! ledger row.
//!
//! Idempotent: a ledger row is consumed by the expansion, so re-running is a
//! no-op. Runs at launch, after every pull that merged the ledger or the track
//! bucket, and directly from `add_tracks_to_release`.

use rusqlite::Connection;

use crate::error::Result;
use crate::services::cloud_sync::hlc;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

struct LedgerRow {
    playlist_id: String,
    release_id: String,
    date_added: Option<String>,
    hlc: String,
}

/// Expand every ledger row whose release now has tracks. Returns the number of
/// ledger rows consumed.
pub fn expand_release_memberships(conn: &Connection) -> Result<usize> {
    expand(conn, None)
}

/// Expand only the ledger rows for `release_id` (called right after tracks are added
/// to a release so a pending membership shows up without waiting for the next launch).
pub fn expand_release_memberships_for(conn: &Connection, release_id: &str) -> Result<usize> {
    expand(conn, Some(release_id))
}

fn expand(conn: &Connection, only_release: Option<&str>) -> Result<usize> {
    let filter = if only_release.is_some() {
        " AND pdr.release_id = ?1"
    } else {
        ""
    };
    let sql = format!(
        "SELECT pdr.playlist_id, pdr.release_id, pdr.date_added, pdr._hlc \
         FROM playlist_discovery_releases pdr \
         WHERE EXISTS (SELECT 1 FROM discovery_tracks dt WHERE dt.release_id = pdr.release_id){filter} \
         ORDER BY pdr.playlist_id, pdr.position, pdr.release_id"
    );
    let ledger: Vec<LedgerRow> = {
        let mut stmt = conn.prepare(&sql)?;
        let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<LedgerRow> {
            Ok(LedgerRow {
                playlist_id: r.get(0)?,
                release_id: r.get(1)?,
                date_added: r.get(2)?,
                hlc: r.get(3)?,
            })
        };
        let rows = match only_release {
            Some(id) => stmt
                .query_map([id], map)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt
                .query_map([], map)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };
        rows
    };
    if ledger.is_empty() {
        return Ok(0);
    }

    let tx = conn.unchecked_transaction()?;
    let mut expanded = 0usize;

    for row in &ledger {
        let track_ids: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM discovery_tracks WHERE release_id = ?1 ORDER BY position, id",
            )?;
            let ids = stmt
                .query_map([&row.release_id], |r| r.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ids
        };
        let next_position: i32 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_discovery_tracks WHERE playlist_id = ?1",
            [&row.playlist_id],
            |r| r.get(0),
        )?;
        // A NULL/empty `date_added` (pre-NOT-NULL ledger rows) falls back to the
        // release's own date — deterministic, so peers expanding the same row agree.
        let date_added: String = match row.date_added.as_deref().filter(|d| !d.is_empty()) {
            Some(d) => d.to_string(),
            None => tx.query_row(
                "SELECT date_added FROM discovery_releases WHERE id = ?1",
                [&row.release_id],
                |r| r.get(0),
            )?,
        };
        let stamped = !row.hlc.is_empty();
        let stamp = if stamped {
            hlc::bump(&row.hlc)
        } else {
            String::new()
        };

        for (i, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO playlist_discovery_tracks \
                 (playlist_id, track_id, position, date_added, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    row.playlist_id,
                    track_id,
                    next_position + i as i32,
                    date_added,
                    stamp
                ],
            )?;
        }
        tx.execute(
            "DELETE FROM playlist_discovery_releases WHERE playlist_id = ?1 AND release_id = ?2",
            rusqlite::params![row.playlist_id, row.release_id],
        )?;
        if stamped {
            dirty::record_tombstone(
                &tx,
                buckets::PLAYLIST_DISCOVERY_RELEASES,
                &dirty::junction_entity_id(&row.playlist_id, &row.release_id),
                &stamp,
            )?;
        }
        expanded += 1;
    }

    dirty::mark_dirty(&tx, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
    dirty::mark_dirty(&tx, buckets::PLAYLIST_DISCOVERY_TRACKS)?;
    tx.commit()?;
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cloud_sync::hlc::Hlc;

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

    fn seed_playlist(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO playlists (id, name, is_folder, is_smart, sort_order, date_created, date_modified, context, _hlc) \
             VALUES (?1, ?1, 0, 0, 0, '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', 'discovery', ?2)",
            rusqlite::params![id, h(1)],
        )
        .unwrap();
    }

    fn seed_release(conn: &Connection, id: &str, track_names: &[&str]) {
        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, date_added, date_modified, _hlc) \
             VALUES (?1, ?2, 'bandcamp', '2021-06-01T00:00:00Z', '2021-06-01T00:00:00Z', ?3)",
            rusqlite::params![id, format!("https://x.bandcamp.com/{id}"), h(1)],
        )
        .unwrap();
        for (i, name) in track_names.iter().enumerate() {
            conn.execute(
                "INSERT INTO discovery_tracks (id, release_id, name, position, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![format!("{id}-t{i}"), id, name, i as i32 + 1, h(1)],
            )
            .unwrap();
        }
    }

    fn seed_ledger(conn: &Connection, playlist: &str, release: &str, position: i32, hlc: &str) {
        conn.execute(
            "INSERT INTO playlist_discovery_releases (playlist_id, release_id, position, date_added, _hlc) \
             VALUES (?1, ?2, ?3, '2022-01-01T00:00:00Z', ?4)",
            rusqlite::params![playlist, release, position, hlc],
        )
        .unwrap();
    }

    fn member_rows(conn: &Connection, playlist: &str) -> Vec<(String, i32, String)> {
        let mut stmt = conn
            .prepare(
                "SELECT track_id, position, _hlc FROM playlist_discovery_tracks \
                 WHERE playlist_id = ?1 ORDER BY position",
            )
            .unwrap();
        stmt.query_map([playlist], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    fn ledger_count(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM playlist_discovery_releases",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn expands_in_release_then_track_order_and_tombstones_ledger() {
        let conn = mem_db();
        seed_playlist(&conn, "p1");
        seed_release(&conn, "r1", &["A1", "A2"]);
        seed_release(&conn, "r2", &["B1"]);
        seed_ledger(&conn, "p1", "r2", 0, &h(10));
        seed_ledger(&conn, "p1", "r1", 1, &h(20));

        assert_eq!(expand_release_memberships(&conn).unwrap(), 2);
        let rows = member_rows(&conn, "p1");
        let ids: Vec<&str> = rows.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            ["r2-t0", "r1-t0", "r1-t1"],
            "release order, then track order"
        );
        assert_eq!(
            rows.iter().map(|(_, p, _)| *p).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(
            rows[0].2,
            hlc::bump(&h(10)),
            "stamp is a bump of the ledger row"
        );
        assert_eq!(rows[1].2, hlc::bump(&h(20)));
        assert_eq!(ledger_count(&conn), 0);

        let tomb: String = conn
            .query_row(
                "SELECT _hlc FROM sync_tombstones WHERE entity_type = 'playlist_discovery_releases' \
                 AND entity_id = 'p1|r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tomb, hlc::bump(&h(20)));
    }

    #[test]
    fn trackless_release_stays_in_ledger_until_tracks_arrive() {
        let conn = mem_db();
        seed_playlist(&conn, "p1");
        seed_release(&conn, "r1", &[]);
        seed_ledger(&conn, "p1", "r1", 0, &h(10));

        assert_eq!(expand_release_memberships(&conn).unwrap(), 0);
        assert_eq!(ledger_count(&conn), 1);

        conn.execute(
            "INSERT INTO discovery_tracks (id, release_id, name, position, _hlc) VALUES ('r1-t0', 'r1', 'A1', 1, ?1)",
            [h(2)],
        )
        .unwrap();
        assert_eq!(expand_release_memberships_for(&conn, "r1").unwrap(), 1);
        assert_eq!(member_rows(&conn, "p1").len(), 1);
        assert_eq!(ledger_count(&conn), 0);
    }

    #[test]
    fn unstamped_ledger_row_yields_unstamped_members_and_no_tombstone() {
        let conn = mem_db();
        seed_playlist(&conn, "p1");
        seed_release(&conn, "r1", &["A1"]);
        seed_ledger(&conn, "p1", "r1", 0, "");

        assert_eq!(expand_release_memberships(&conn).unwrap(), 1);
        assert_eq!(member_rows(&conn, "p1")[0].2, "");
        let tombs: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_tombstones", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tombs, 0);
    }

    #[test]
    fn second_run_is_a_noop_and_appends_after_existing_members() {
        let conn = mem_db();
        seed_playlist(&conn, "p1");
        seed_release(&conn, "r1", &["A1"]);
        seed_release(&conn, "r2", &["B1"]);
        seed_ledger(&conn, "p1", "r1", 0, &h(10));
        assert_eq!(expand_release_memberships(&conn).unwrap(), 1);
        assert_eq!(expand_release_memberships(&conn).unwrap(), 0);

        seed_ledger(&conn, "p1", "r2", 0, &h(30));
        assert_eq!(expand_release_memberships(&conn).unwrap(), 1);
        let rows = member_rows(&conn, "p1");
        assert_eq!(rows[1].0, "r2-t0");
        assert_eq!(
            rows[1].1, 1,
            "later ledger rows append after existing members"
        );
    }
}
