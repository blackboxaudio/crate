//! Content-based duplicate collapse for discovery entities.
//!
//! Discovery rows have a natural key the id-keyed merge can't see: a track is
//! identified by `(release_id, normalized name)` and a release by its normalized
//! URL. Historically both minted random v4 ids, so two devices independently
//! materializing the same content (e.g. each running "refresh metadata" on a
//! release that synced without tracks) produced disjoint id sets that the merge
//! unioned into visible duplicates — or, for releases, a permanent id split-brain
//! behind the UNIQUE(url) skip. New rows now mint content-derived (v5) ids, and
//! this module heals the state that already exists in the cloud: when a remote
//! live row's id is unknown locally but a local live row carries the same natural
//! key, the two are collapsed into one.
//!
//! ## The convergence rule (every step must be a pure function of the two rows)
//!
//! - **Winner `W`** = the row with the larger HLC (exact ties — impossible across
//!   nodes, defensive only — go to the smaller id).
//! - **Survivor id** = the smaller of the two ids; the larger id is tombstoned.
//! - **Merged fields**: `W`'s row, with `NULL`s coalesced from the loser; flags
//!   union across both (`is_liked` ORs; a release stays `is_new` only if BOTH
//!   sides still have it new).
//! - **Survivor HLC**: if the merged row IS `W` (same id, same fields), keep `W`'s
//!   HLC unchanged — this makes re-encounters with stale copies a fixed point
//!   instead of an endless escalation. Otherwise `hlc::bump(W's HLC)`: strictly
//!   greater than both inputs (so it wins LWW everywhere) yet derived without any
//!   local clock (so every device computes the same stamp and the serialized
//!   buckets converge byte-for-byte).
//! - **Tombstone HLC** = survivor HLC (>= every live copy of the loser, and
//!   entities are DELETE-WINS-TIE).

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{CrateError, Result};
use crate::models::{normalized_track_name, DiscoveryTrack};
use crate::services::cloud_sync::hlc;

use super::super::buckets::Bucket;
use super::super::rows::{DiscoveryReleaseRow, ParsedRow};
use super::writers;

/// Try to collapse a LIVE remote row whose id has no local live row into a local
/// live row with the same natural key. Returns `Ok(true)` iff a collapse was
/// applied (the caller then skips its plain insert/resurrect).
pub(super) fn try_collapse(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<bool> {
    match bucket {
        Bucket::DiscoveryTracks => collapse_track(tx, bucket, row),
        Bucket::DiscoveryReleases => collapse_release(tx, bucket, row),
        _ => Ok(false),
    }
}

fn de<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Result<T> {
    serde_json::from_value(value.clone())
        .map_err(|e| CrateError::CloudSync(format!("deserialize collapse row: {e}")))
}

/// `(winner, winner_hlc, loser, loser_hlc)` by the pure ordering rule above.
fn pick_winner<'a, T>(
    l: &'a T,
    l_hlc: &'a str,
    l_id: &str,
    r: &'a T,
    r_hlc: &'a str,
    r_id: &str,
) -> (&'a T, &'a str, &'a T, &'a str) {
    let l_wins = match l_hlc.cmp(r_hlc) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => l_id < r_id,
    };
    if l_wins {
        (l, l_hlc, r, r_hlc)
    } else {
        (r, r_hlc, l, l_hlc)
    }
}

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------

pub(super) struct CollapsedTrack {
    pub survivor: DiscoveryTrack,
    pub survivor_hlc: String,
    pub loser_id: String,
    pub tombstone_hlc: String,
}

/// Pure, symmetric pairwise collapse of two same-`(release, normalized name)` track
/// rows. Argument order MUST NOT affect the result.
pub(super) fn collapse_track_pair(
    l: &DiscoveryTrack,
    l_hlc: &str,
    r: &DiscoveryTrack,
    r_hlc: &str,
) -> CollapsedTrack {
    let (w, w_hlc, lo, _) = pick_winner(l, l_hlc, &l.id, r, r_hlc, &r.id);
    let survivor_id = std::cmp::min(&l.id, &r.id).clone();
    let loser_id = std::cmp::max(&l.id, &r.id).clone();
    let survivor = DiscoveryTrack {
        id: survivor_id,
        release_id: w.release_id.clone(),
        name: w.name.clone(),
        position: w.position,
        duration_ms: w.duration_ms.or(lo.duration_ms),
        video_id: w.video_id.clone().or_else(|| lo.video_id.clone()),
        url: w.url.clone().or_else(|| lo.url.clone()),
        is_liked: l.is_liked || r.is_liked,
    };
    let survivor_hlc = if survivor == *w {
        w_hlc.to_string()
    } else {
        hlc::bump(w_hlc)
    };
    CollapsedTrack {
        loser_id,
        tombstone_hlc: survivor_hlc.clone(),
        survivor,
        survivor_hlc,
    }
}

fn collapse_track(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<bool> {
    let remote: DiscoveryTrack = de(&row.value)?;
    let want = normalized_track_name(&remote.name);

    // Smallest-id live sibling with the same normalized name (Rust-side comparison —
    // SQL LOWER() is ASCII-only and must not decide identity).
    let mut stmt = tx.prepare(
        "SELECT id, release_id, name, position, duration_ms, video_id, url, is_liked, _hlc \
         FROM discovery_tracks WHERE release_id = ?1 ORDER BY id",
    )?;
    let siblings: Vec<(DiscoveryTrack, String)> = stmt
        .query_map([&remote.release_id], |r| {
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
    let Some((local, local_hlc)) = siblings
        .into_iter()
        .find(|(t, _)| t.id != remote.id && normalized_track_name(&t.name) == want)
    else {
        return Ok(false);
    };

    let c = collapse_track_pair(&local, &local_hlc, &remote, &row.hlc);
    if c.survivor.id == local.id {
        // Local id survives: fold the remote copy in, tombstone the remote id.
        tx.execute(
            "UPDATE discovery_tracks SET name = ?1, position = ?2, duration_ms = ?3, \
             video_id = ?4, url = ?5, is_liked = ?6, _hlc = ?7 WHERE id = ?8",
            params![
                c.survivor.name,
                c.survivor.position,
                c.survivor.duration_ms,
                c.survivor.video_id,
                c.survivor.url,
                c.survivor.is_liked,
                c.survivor_hlc,
                local.id,
            ],
        )?;
    } else {
        // Remote id survives: replace the local row wholesale. Clearing any tombstone on
        // the survivor mirrors the resurrect arm (the reverse-direction collapse on a
        // peer may have tombstoned this very id before this device healed).
        tx.execute("DELETE FROM discovery_tracks WHERE id = ?1", [&local.id])?;
        writers::delete_tombstone(tx, bucket, &c.survivor.id)?;
        writers::upsert_discovery_track(tx, &c.survivor, &c.survivor_hlc)?;
    }
    writers::upsert_tombstone(tx, bucket, &c.loser_id, &c.tombstone_hlc)?;
    log::info!(
        "cloud_sync merge: collapsed duplicate discovery track {:?} ({} -> {})",
        c.survivor.name,
        c.loser_id,
        c.survivor.id
    );
    Ok(true)
}

// ---------------------------------------------------------------------------
// Releases
// ---------------------------------------------------------------------------

pub(super) struct CollapsedRelease {
    pub survivor: DiscoveryReleaseRow,
    pub survivor_hlc: String,
    pub loser_id: String,
    pub tombstone_hlc: String,
}

/// Pure, symmetric pairwise collapse of two same-URL release rows.
pub(super) fn collapse_release_pair(
    l: &DiscoveryReleaseRow,
    l_hlc: &str,
    r: &DiscoveryReleaseRow,
    r_hlc: &str,
) -> CollapsedRelease {
    let (w, w_hlc, lo, _) = pick_winner(l, l_hlc, &l.id, r, r_hlc, &r.id);
    let survivor_id = std::cmp::min(&l.id, &r.id).clone();
    let loser_id = std::cmp::max(&l.id, &r.id).clone();
    let survivor = DiscoveryReleaseRow {
        id: survivor_id,
        url: w.url.clone(),
        source_type: w.source_type.clone(),
        artist: w.artist.clone().or_else(|| lo.artist.clone()),
        title: w.title.clone().or_else(|| lo.title.clone()),
        label: w.label.clone().or_else(|| lo.label.clone()),
        release_date: w.release_date.clone().or_else(|| lo.release_date.clone()),
        artwork_url: w.artwork_url.clone().or_else(|| lo.artwork_url.clone()),
        artwork_path: w.artwork_path.clone().or_else(|| lo.artwork_path.clone()),
        notes: w.notes.clone().or_else(|| lo.notes.clone()),
        parent_url: w.parent_url.clone().or_else(|| lo.parent_url.clone()),
        source_page_url: w
            .source_page_url
            .clone()
            .or_else(|| lo.source_page_url.clone()),
        // min/max on verbatim RFC3339 strings: deterministic, and "first added / last
        // modified" is the honest union of the two histories.
        date_added: std::cmp::min(&l.date_added, &r.date_added).clone(),
        date_modified: std::cmp::max(&l.date_modified, &r.date_modified).clone(),
        // "New/unreviewed" only while NEITHER device has reviewed it.
        is_new: l.is_new && r.is_new,
        surfaced_at: w.surfaced_at.clone().or_else(|| lo.surfaced_at.clone()),
    };
    let survivor_hlc = if survivor == *w {
        w_hlc.to_string()
    } else {
        hlc::bump(w_hlc)
    };
    CollapsedRelease {
        loser_id,
        tombstone_hlc: survivor_hlc.clone(),
        survivor,
        survivor_hlc,
    }
}

fn collapse_release(tx: &Connection, bucket: &Bucket, row: &ParsedRow) -> Result<bool> {
    let remote: DiscoveryReleaseRow = de(&row.value)?;

    // UNIQUE(url) means at most one local live row can carry this URL. URLs are stored
    // normalized by `create_release`, so exact (indexed) equality is the natural key.
    let local: Option<(DiscoveryReleaseRow, String)> = tx
        .query_row(
            "SELECT id, url, source_type, artist, title, label, release_date, artwork_url, \
             artwork_path, notes, parent_url, source_page_url, date_added, date_modified, \
             is_new, surfaced_at, _hlc \
             FROM discovery_releases WHERE url = ?1 AND id != ?2",
            params![remote.url, remote.id],
            |r| {
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
            },
        )
        .optional()?;
    let Some((local, local_hlc)) = local else {
        return Ok(false);
    };

    let c = collapse_release_pair(&local, &local_hlc, &remote, &row.hlc);
    if c.survivor.id == local.id {
        // Local id survives; the remote id is tombstoned. The remote's children still
        // reference the loser id and will be parent-skipped this pull — the pushing
        // device re-parents them when it applies this same collapse, so they arrive
        // under the survivor on the next cycle.
        tx.execute(
            "UPDATE discovery_releases SET source_type = ?1, artist = ?2, title = ?3, \
             label = ?4, release_date = ?5, artwork_url = ?6, artwork_path = ?7, notes = ?8, \
             parent_url = ?9, source_page_url = ?10, date_added = ?11, date_modified = ?12, \
             is_new = ?13, surfaced_at = ?14, _hlc = ?15 WHERE id = ?16",
            params![
                c.survivor.source_type,
                c.survivor.artist,
                c.survivor.title,
                c.survivor.label,
                c.survivor.release_date,
                c.survivor.artwork_url,
                c.survivor.artwork_path,
                c.survivor.notes,
                c.survivor.parent_url,
                c.survivor.source_page_url,
                c.survivor.date_added,
                c.survivor.date_modified,
                c.survivor.is_new,
                c.survivor.surfaced_at,
                c.survivor_hlc,
                local.id,
            ],
        )?;
    } else {
        // Remote id survives: re-key the local row and re-parent every child. FK checks
        // are deferred to COMMIT (merge_bucket sets the pragma), so the intermediate
        // states are fine.
        tx.execute(
            "UPDATE discovery_releases SET id = ?1, source_type = ?2, artist = ?3, title = ?4, \
             label = ?5, release_date = ?6, artwork_url = ?7, artwork_path = ?8, notes = ?9, \
             parent_url = ?10, source_page_url = ?11, date_added = ?12, date_modified = ?13, \
             is_new = ?14, surfaced_at = ?15, _hlc = ?16 WHERE id = ?17",
            params![
                c.survivor.id,
                c.survivor.source_type,
                c.survivor.artist,
                c.survivor.title,
                c.survivor.label,
                c.survivor.release_date,
                c.survivor.artwork_url,
                c.survivor.artwork_path,
                c.survivor.notes,
                c.survivor.parent_url,
                c.survivor.source_page_url,
                c.survivor.date_added,
                c.survivor.date_modified,
                c.survivor.is_new,
                c.survivor.surfaced_at,
                c.survivor_hlc,
                local.id,
            ],
        )?;
        // Tracks keep their own ids — only the parent pointer moves. Same-name pairs
        // that later arrive under the survivor collapse via collapse_track.
        tx.execute(
            "UPDATE discovery_tracks SET release_id = ?1 WHERE release_id = ?2",
            params![c.survivor.id, local.id],
        )?;
        // Junctions embed the release id in their PK: move what can move, drop the
        // leftovers whose (new-id, other-id) pair already exists.
        for table in [
            "discovery_release_tags",
            "playlist_discovery_releases",
            "discovery_release_sources",
        ] {
            tx.execute(
                &format!("UPDATE OR IGNORE {table} SET release_id = ?1 WHERE release_id = ?2"),
                params![c.survivor.id, local.id],
            )?;
            tx.execute(
                &format!("DELETE FROM {table} WHERE release_id = ?1"),
                [&local.id],
            )?;
        }
        // Device-local caches are keyed by the old id (including on-disk file names):
        // drop the rows and let them re-fill on demand.
        for table in [
            "discovery_stream_cache",
            "discovery_audio_cache",
            "discovery_artwork_cache",
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE release_id = ?1"),
                [&local.id],
            )?;
        }
        // Mirror the resurrect arm: a peer's reverse-direction collapse may have
        // tombstoned the surviving id before this device healed.
        writers::delete_tombstone(tx, bucket, &c.survivor.id)?;
    }
    writers::upsert_tombstone(tx, bucket, &c.loser_id, &c.tombstone_hlc)?;
    log::info!(
        "cloud_sync merge: collapsed duplicate discovery release {:?} ({} -> {})",
        c.survivor.url,
        c.loser_id,
        c.survivor.id
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, liked: bool, duration_ms: Option<i64>, url: Option<&str>) -> DiscoveryTrack {
        DiscoveryTrack {
            id: id.into(),
            release_id: "rel".into(),
            name: "Intro".into(),
            position: 1,
            duration_ms,
            video_id: None,
            url: url.map(str::to_string),
            is_liked: liked,
        }
    }

    fn h(wall: u64) -> String {
        hlc::Hlc::new(wall, 0, 0xA).format()
    }

    #[test]
    fn track_pair_is_symmetric() {
        let l = track("aaaa", false, Some(1000), None);
        let r = track("bbbb", true, None, Some("https://t"));
        let c1 = collapse_track_pair(&l, &h(10), &r, &h(20));
        let c2 = collapse_track_pair(&r, &h(20), &l, &h(10));
        assert_eq!(c1.survivor, c2.survivor);
        assert_eq!(c1.survivor_hlc, c2.survivor_hlc);
        assert_eq!(c1.loser_id, c2.loser_id);
        assert_eq!(c1.tombstone_hlc, c2.tombstone_hlc);
    }

    #[test]
    fn track_pair_ors_likes_and_coalesces_nulls() {
        let l = track("aaaa", false, Some(1000), None);
        let r = track("bbbb", true, None, Some("https://t"));
        let c = collapse_track_pair(&l, &h(10), &r, &h(20));
        assert_eq!(c.survivor.id, "aaaa");
        assert_eq!(c.loser_id, "bbbb");
        assert!(c.survivor.is_liked, "like ORs across both copies");
        assert_eq!(c.survivor.duration_ms, Some(1000), "loser's non-null kept");
        assert_eq!(c.survivor.url.as_deref(), Some("https://t"));
    }

    #[test]
    fn bump_case_is_strictly_greater_than_both_inputs() {
        let l = track("aaaa", false, None, None);
        let r = track("bbbb", true, None, None);
        let c = collapse_track_pair(&l, &h(10), &r, &h(20));
        assert!(c.survivor_hlc > h(20));
        assert!(c.survivor_hlc > h(10));
        assert_eq!(c.tombstone_hlc, c.survivor_hlc);
    }

    #[test]
    fn already_merged_winner_is_a_fixed_point() {
        // Winner has the smaller id and already carries the union of everything —
        // re-encountering a stale copy of the loser must not escalate the HLC.
        let w = track("aaaa", true, Some(1000), Some("https://t"));
        let stale = track("bbbb", false, None, None);
        let c = collapse_track_pair(&w, &h(20), &stale, &h(10));
        assert_eq!(c.survivor, w);
        assert_eq!(c.survivor_hlc, h(20), "no bump when nothing changed");
    }

    fn release(id: &str, artist: Option<&str>, added: &str, is_new: bool) -> DiscoveryReleaseRow {
        DiscoveryReleaseRow {
            id: id.into(),
            url: "https://x.bandcamp.com/album/y".into(),
            source_type: "bandcamp".into(),
            artist: artist.map(str::to_string),
            title: None,
            label: None,
            release_date: None,
            artwork_url: None,
            artwork_path: None,
            notes: None,
            parent_url: None,
            source_page_url: None,
            date_added: added.into(),
            date_modified: added.into(),
            is_new,
            surfaced_at: None,
        }
    }

    #[test]
    fn release_pair_is_symmetric_and_unions_history() {
        let l = release("p-rel", Some("Artist"), "2020-01-01T00:00:00Z", true);
        let r = release("q-rel", None, "2019-06-01T00:00:00Z", false);
        let c1 = collapse_release_pair(&l, &h(10), &r, &h(20));
        let c2 = collapse_release_pair(&r, &h(20), &l, &h(10));
        assert_eq!(c1.survivor, c2.survivor);
        assert_eq!(c1.survivor_hlc, c2.survivor_hlc);
        assert_eq!(c1.survivor.id, "p-rel");
        assert_eq!(c1.loser_id, "q-rel");
        assert_eq!(
            c1.survivor.artist.as_deref(),
            Some("Artist"),
            "null coalesced"
        );
        assert_eq!(
            c1.survivor.date_added, "2019-06-01T00:00:00Z",
            "earliest added"
        );
        assert_eq!(
            c1.survivor.date_modified, "2020-01-01T00:00:00Z",
            "latest modified"
        );
        assert!(!c1.survivor.is_new, "reviewed on either device -> not new");
    }
}
