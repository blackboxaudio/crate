//! One-shot heal for historically unnormalized discovery-release URLs.
//!
//! Label-page scans used to store hrefs verbatim — query strings
//! (`?label=…&tab=music`) and HTML-entity ampersands included — so the same
//! release could exist under several URL spellings, and collection/ownership
//! matching (exact URL identity) missed all of them. `normalize_url` now
//! canonicalizes these at write time; this sweep re-normalizes the rows already
//! sitting in the local database.
//!
//! Runs on every launch, before the track dedupe sweep (a merge here can leave
//! same-name track pairs under one release for that sweep to collapse). It is
//! one scan of the releases table and a no-op once URLs are clean.
//!
//! Collapse semantics mirror the merge engine's release collapse: when several
//! rows normalize to the same URL, keep the smallest id, fold fields
//! winner-first (highest `_hlc` wins, `NULL`s coalesced, `is_new` ANDs),
//! re-parent children, and tombstone the removed ids so peers drop them too.
//! Rows whose clean URL is unclaimed are updated in place, keeping their id —
//! peers converge by running the same sweep, and a straggler pushing the old
//! spelling collapses against this row by URL in the sync merge.
//!
//! Every stamp written here is `hlc::bump` of a stamp already on a row in the
//! group — NEVER a fresh clock reading (see `dedupe.rs` for why minimal
//! dominance is load-bearing).

use rusqlite::Connection;

use super::normalize_url;
use crate::error::Result;
use crate::services::cloud_sync::hlc;
use crate::services::cloud_sync::pipeline::rows::DiscoveryReleaseRow;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

/// Junction tables whose rows must follow a merged release to the survivor id.
const RELEASE_JUNCTIONS: &[&str] = &[
    "discovery_release_tags",
    "playlist_discovery_releases",
    "discovery_release_sources",
];

/// Device-local caches keyed by release id: drop rows for removed ids and let
/// them re-fill on demand.
const RELEASE_LOCAL_CACHES: &[&str] = &[
    "discovery_stream_cache",
    "discovery_audio_cache",
    "discovery_artwork_cache",
    "discovery_preview_unavailable",
];

fn load_releases(conn: &Connection) -> Result<Vec<(DiscoveryReleaseRow, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, \
         artwork_path, notes, parent_url, source_page_url, date_added, date_modified, \
         is_new, surfaced_at, _hlc FROM discovery_releases ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                DiscoveryReleaseRow {
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
                    source_page_url: r.get(11)?,
                    date_added: r.get(12)?,
                    date_modified: r.get(13)?,
                    is_new: r.get::<_, i32>(14).map(|v| v != 0)?,
                    surfaced_at: r.get(15)?,
                },
                r.get::<_, String>(16)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Re-normalize `discovery_releases.url` for every stored row, merging rows that
/// collapse onto the same clean URL. Returns `(updated, merged_away)` row counts.
/// Idempotent; safe to run every launch.
pub fn renormalize_release_urls(conn: &Connection) -> Result<(usize, usize)> {
    let all = load_releases(conn)?;

    // Group every row that needs to move — plus the row already holding the clean
    // URL, if any — by that clean URL. BTreeMap for deterministic sweep order.
    let mut groups: std::collections::BTreeMap<String, Vec<(DiscoveryReleaseRow, String)>> =
        std::collections::BTreeMap::new();
    for (row, row_hlc) in &all {
        let clean = normalize_url(&row.url);
        if clean != row.url {
            groups
                .entry(clean)
                .or_default()
                .push((row.clone(), row_hlc.clone()));
        }
    }
    if groups.is_empty() {
        return Ok((0, 0));
    }
    for (row, row_hlc) in &all {
        if let Some(members) = groups.get_mut(&row.url) {
            members.push((row.clone(), row_hlc.clone()));
        }
    }

    let tx = conn.unchecked_transaction()?;
    let mut updated = 0usize;
    let mut merged_away = 0usize;
    let mut repointed_tracks = false;

    for (clean_url, mut members) in groups {
        // Winner-first: highest `_hlc`, ties to the smaller id (same rule as the
        // merge collapse). The smallest id survives regardless of who wins, which
        // makes independent sweeps on two devices commute.
        members.sort_by(|(a, a_hlc), (b, b_hlc)| b_hlc.cmp(a_hlc).then_with(|| a.id.cmp(&b.id)));
        let group_max_hlc = members[0].1.clone();
        let survivor_id = members
            .iter()
            .map(|(r, _)| r.id.clone())
            .min()
            .expect("non-empty group");

        let mut merged = members[0].0.clone();
        for (r, _) in members.iter().skip(1) {
            merged.artist = merged.artist.or_else(|| r.artist.clone());
            merged.title = merged.title.or_else(|| r.title.clone());
            merged.label = merged.label.or_else(|| r.label.clone());
            merged.release_date = merged.release_date.or_else(|| r.release_date.clone());
            merged.artwork_url = merged.artwork_url.or_else(|| r.artwork_url.clone());
            merged.artwork_path = merged.artwork_path.or_else(|| r.artwork_path.clone());
            merged.notes = merged.notes.or_else(|| r.notes.clone());
            merged.parent_url = merged.parent_url.or_else(|| r.parent_url.clone());
            merged.source_page_url = merged.source_page_url.or_else(|| r.source_page_url.clone());
            merged.date_added = std::cmp::min(merged.date_added, r.date_added.clone());
            merged.date_modified = std::cmp::max(merged.date_modified, r.date_modified.clone());
            merged.is_new = merged.is_new && r.is_new;
        }
        merged.id = survivor_id.clone();
        merged.url = clean_url;

        let survivor_row = members
            .iter()
            .find(|(r, _)| r.id == survivor_id)
            .expect("survivor is a member")
            .0
            .clone();

        // Remove losers first: their tracks and junction rows move to the survivor
        // (tracks CASCADE on release delete, so re-parent before deleting), and the
        // survivor's url UPDATE below must not hit UNIQUE(url) against them.
        for (r, r_hlc) in members.iter().filter(|(r, _)| r.id != survivor_id) {
            let moved = tx.execute(
                "UPDATE discovery_tracks SET release_id = ?1 WHERE release_id = ?2",
                rusqlite::params![survivor_id, r.id],
            )?;
            repointed_tracks = repointed_tracks || moved > 0;
            // Junctions embed the release id in their PK: move what can move, drop
            // the leftovers whose (new-id, other-id) pair already exists.
            for table in RELEASE_JUNCTIONS {
                tx.execute(
                    &format!("UPDATE OR IGNORE {table} SET release_id = ?1 WHERE release_id = ?2"),
                    rusqlite::params![survivor_id, r.id],
                )?;
                tx.execute(
                    &format!("DELETE FROM {table} WHERE release_id = ?1"),
                    [&r.id],
                )?;
            }
            for table in RELEASE_LOCAL_CACHES {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE release_id = ?1"),
                    [&r.id],
                )?;
            }
            tx.execute("DELETE FROM discovery_releases WHERE id = ?1", [&r.id])?;
            // Strictly above THIS copy (kills identical copies on peers), but
            // minimally so — a peer's later edit outranks the tombstone, survives,
            // and the merge collapse folds it into the survivor on the next sync.
            dirty::record_tombstone(&tx, buckets::DISCOVERY_RELEASES, &r.id, &hlc::bump(r_hlc))?;
            merged_away += 1;
        }

        // Re-stamp only on content change; the stamp outranks every copy in this
        // group (so the merged content wins LWW against them on peers) but nothing
        // newer.
        if merged != survivor_row {
            let stamp = hlc::bump(&group_max_hlc);
            tx.execute(
                "UPDATE discovery_releases SET url = ?1, source_type = ?2, artist = ?3, \
                 title = ?4, label = ?5, release_date = ?6, artwork_url = ?7, \
                 artwork_path = ?8, notes = ?9, parent_url = ?10, source_page_url = ?11, \
                 date_added = ?12, date_modified = ?13, is_new = ?14, surfaced_at = ?15, \
                 _hlc = ?16 WHERE id = ?17",
                rusqlite::params![
                    merged.url,
                    merged.source_type,
                    merged.artist,
                    merged.title,
                    merged.label,
                    merged.release_date,
                    merged.artwork_url,
                    merged.artwork_path,
                    merged.notes,
                    merged.parent_url,
                    merged.source_page_url,
                    merged.date_added,
                    merged.date_modified,
                    merged.is_new,
                    merged.surfaced_at,
                    stamp,
                    survivor_id,
                ],
            )?;
            updated += 1;
        }
    }

    dirty::mark_dirty(&tx, buckets::DISCOVERY_RELEASES)?;
    if repointed_tracks {
        // Re-parented tracks keep their ids and stamps (peers re-parent via their
        // own sweep — same rule as the merge collapse); the mark just schedules a
        // push cycle so tombstones and the survivor rows propagate promptly.
        dirty::mark_dirty(&tx, buckets::DISCOVERY_TRACKS)?;
    }
    tx.commit()?;
    Ok((updated, merged_away))
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

    fn seed_release(conn: &Connection, id: &str, url: &str, artist: Option<&str>, wall: u64) {
        conn.execute(
            "INSERT INTO discovery_releases (id, url, source_type, artist, date_added, date_modified, _hlc) \
             VALUES (?1, ?2, 'bandcamp', ?3, '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', ?4)",
            rusqlite::params![id, url, artist, h(wall)],
        )
        .unwrap();
    }

    fn seed_track(conn: &Connection, id: &str, release: &str, name: &str) {
        conn.execute(
            "INSERT INTO discovery_tracks (id, release_id, name, position, _hlc) \
             VALUES (?1, ?2, ?3, 1, ?4)",
            rusqlite::params![id, release, name, h(1)],
        )
        .unwrap();
    }

    fn release_urls(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT id, url FROM discovery_releases ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn clean_db_is_a_noop() {
        let conn = mem_db();
        seed_release(&conn, "rel-1", "https://a.bandcamp.com/album/x", None, 10);
        assert_eq!(renormalize_release_urls(&conn).unwrap(), (0, 0));
    }

    #[test]
    fn dirty_url_updated_in_place_keeping_id() {
        let conn = mem_db();
        seed_release(
            &conn,
            "rel-1",
            "https://a.bandcamp.com/album/x?label=123&amp;tab=music",
            Some("Artist"),
            10,
        );
        assert_eq!(renormalize_release_urls(&conn).unwrap(), (1, 0));
        assert_eq!(
            release_urls(&conn),
            vec![(
                "rel-1".to_string(),
                "https://a.bandcamp.com/album/x".to_string()
            )]
        );
        let hlc: String = conn
            .query_row(
                "SELECT _hlc FROM discovery_releases WHERE id = 'rel-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(hlc > h(10), "changed row re-stamped above its old stamp");
        assert!(hlc < h(11), "minimal dominance: below the next wall tick");
    }

    #[test]
    fn dirty_row_merges_into_existing_clean_twin() {
        let conn = mem_db();
        // Clean twin has the smaller id and older stamp; dirty row carries the artist.
        seed_release(&conn, "aaaa", "https://a.bandcamp.com/album/x", None, 10);
        seed_release(
            &conn,
            "bbbb",
            "https://a.bandcamp.com/album/x?label=1&amp;tab=music",
            Some("Artist"),
            20,
        );
        seed_track(&conn, "t-clean", "aaaa", "Intro");
        seed_track(&conn, "t-dirty", "bbbb", "Outro");

        assert_eq!(renormalize_release_urls(&conn).unwrap(), (1, 1));
        assert_eq!(
            release_urls(&conn),
            vec![(
                "aaaa".to_string(),
                "https://a.bandcamp.com/album/x".to_string()
            )]
        );
        let artist: Option<String> = conn
            .query_row(
                "SELECT artist FROM discovery_releases WHERE id = 'aaaa'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            artist.as_deref(),
            Some("Artist"),
            "winner's field folded into survivor"
        );
        let tracks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM discovery_tracks WHERE release_id = 'aaaa'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tracks, 2, "loser's tracks re-parented, not cascaded away");
        let tomb: String = conn
            .query_row(
                "SELECT _hlc FROM sync_tombstones WHERE entity_type = 'discovery_releases' \
                 AND entity_id = 'bbbb'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(tomb > h(20) && tomb < h(21), "minimal-dominance tombstone");
    }

    #[test]
    fn two_dirty_spellings_collapse_onto_one_row() {
        let conn = mem_db();
        seed_release(
            &conn,
            "cccc",
            "https://a.bandcamp.com/album/x?label=1",
            None,
            10,
        );
        seed_release(
            &conn,
            "dddd",
            "https://a.bandcamp.com/album/x?tab=music",
            None,
            20,
        );
        assert_eq!(renormalize_release_urls(&conn).unwrap(), (1, 1));
        assert_eq!(
            release_urls(&conn),
            vec![(
                "cccc".to_string(),
                "https://a.bandcamp.com/album/x".to_string()
            )]
        );
    }

    #[test]
    fn second_run_changes_nothing() {
        let conn = mem_db();
        seed_release(
            &conn,
            "rel-1",
            "https://a.bandcamp.com/album/x?label=123",
            None,
            10,
        );
        seed_release(&conn, "rel-2", "https://b.bandcamp.com/album/y", None, 10);
        assert_eq!(renormalize_release_urls(&conn).unwrap(), (1, 0));
        assert_eq!(renormalize_release_urls(&conn).unwrap(), (0, 0));
    }
}
