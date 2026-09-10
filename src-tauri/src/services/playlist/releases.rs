//! Discovery playlist membership.
//!
//! Membership is per TRACK (`playlist_discovery_tracks`) while the read side stays
//! per RELEASE: `get_playlist_releases` returns the same `DiscoveryRelease[]` shape
//! the feed uses, grouped by release with `tracks` filtered to the member tracks (and
//! `total_track_count` carrying the release's full count). The release-level
//! functions keep their signatures and expand server-side — adding a release adds all
//! of its tracks, removing one removes whatever member tracks it has — so most UI
//! call sites never see the change. A release with no tracks yet cannot be
//! expanded; it is parked in the `playlist_discovery_releases` ledger until the
//! expansion sweep (`expansion.rs`) converts it.

use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

/// One member row as read for grouping: (track_id, release_id, playlist position).
type MemberRow = (String, String, i32);

impl PlaylistService {
    /// Add whole releases: every track of each release becomes a member. Releases whose
    /// tracks have not been fetched yet are parked in the pending-expansion ledger.
    pub fn add_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;

        let mut track_ids: Vec<String> = Vec::new();
        let mut trackless: Vec<&str> = Vec::new();
        for release_id in &release_ids {
            let mut stmt = conn.prepare(
                "SELECT id FROM discovery_tracks WHERE release_id = ?1 ORDER BY position, id",
            )?;
            let ids: Vec<String> = stmt
                .query_map([release_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if ids.is_empty() {
                trackless.push(release_id);
            } else {
                track_ids.extend(ids);
            }
        }

        insert_members(&conn, playlist_id, &track_ids, &now, &hlc)?;

        if !trackless.is_empty() {
            let max_position: i32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(position), -1) FROM playlist_discovery_releases WHERE playlist_id = ?1",
                    [playlist_id],
                    |row| row.get(0),
                )
                .unwrap_or(-1);
            for (i, release_id) in trackless.iter().enumerate() {
                conn.execute(
                    "INSERT OR IGNORE INTO playlist_discovery_releases (playlist_id, release_id, position, date_added, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![playlist_id, release_id, max_position + 1 + i as i32, now, hlc],
                )?;
            }
            dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
        }

        touch_playlist(&conn, playlist_id, &now, &hlc)?;
        drop(conn);
        self.get_playlist(playlist_id)
    }

    /// Add individual tracks (the "this track, not the whole release" path).
    pub fn add_discovery_tracks(
        &self,
        playlist_id: &str,
        track_ids: Vec<String>,
    ) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;

        insert_members(&conn, playlist_id, &track_ids, &now, &hlc)?;
        touch_playlist(&conn, playlist_id, &now, &hlc)?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    /// Remove whole releases: every member track of each release, plus any pending
    /// ledger row.
    pub fn remove_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;

        let mut track_ids: Vec<String> = Vec::new();
        for release_id in &release_ids {
            let mut stmt = conn.prepare(
                "SELECT pdt.track_id FROM playlist_discovery_tracks pdt \
                 JOIN discovery_tracks dt ON dt.id = pdt.track_id \
                 WHERE pdt.playlist_id = ?1 AND dt.release_id = ?2",
            )?;
            let ids: Vec<String> = stmt
                .query_map(rusqlite::params![playlist_id, release_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            track_ids.extend(ids);

            let deleted = conn.execute(
                "DELETE FROM playlist_discovery_releases WHERE playlist_id = ?1 AND release_id = ?2",
                rusqlite::params![playlist_id, release_id],
            )?;
            if deleted > 0 {
                dirty::record_tombstone(
                    &conn,
                    buckets::PLAYLIST_DISCOVERY_RELEASES,
                    &dirty::junction_entity_id(playlist_id, release_id),
                    &hlc,
                )?;
                dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
            }
        }

        delete_members(&conn, playlist_id, &track_ids, &hlc)?;
        compact_positions(&conn, playlist_id, &hlc)?;
        touch_playlist(&conn, playlist_id, &chrono::Utc::now().to_rfc3339(), &hlc)?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    pub fn remove_discovery_tracks(
        &self,
        playlist_id: &str,
        track_ids: Vec<String>,
    ) -> Result<Playlist> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;

        delete_members(&conn, playlist_id, &track_ids, &hlc)?;
        compact_positions(&conn, playlist_id, &hlc)?;
        touch_playlist(&conn, playlist_id, &chrono::Utc::now().to_rfc3339(), &hlc)?;

        drop(conn);
        self.get_playlist(playlist_id)
    }

    /// Reorder the release GROUPS. Member tracks are renumbered so each release's tracks
    /// sit together in the requested group order (a group's internal order is kept);
    /// groups not mentioned keep their relative order after the mentioned ones.
    pub fn reorder_releases(&self, playlist_id: &str, release_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;

        let members = load_members(&conn, playlist_id)?;
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for (track_id, release_id, _) in &members {
            match groups.iter_mut().find(|(rid, _)| rid == release_id) {
                Some((_, ids)) => ids.push(track_id.clone()),
                None => groups.push((release_id.clone(), vec![track_id.clone()])),
            }
        }
        let rank = |rid: &str| -> usize {
            release_ids
                .iter()
                .position(|r| r == rid)
                .unwrap_or(release_ids.len())
        };
        groups.sort_by_key(|(rid, _)| rank(rid));

        let mut position = 0i32;
        for (_, track_ids) in &groups {
            for track_id in track_ids {
                conn.execute(
                    "UPDATE playlist_discovery_tracks SET position = ?1, _hlc = ?2 WHERE playlist_id = ?3 AND track_id = ?4",
                    rusqlite::params![position, hlc, playlist_id, track_id],
                )?;
                position += 1;
            }
        }
        dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_TRACKS)?;

        // Pending (trackless) releases are ordered among themselves on their own scale.
        let ledger: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT release_id FROM playlist_discovery_releases WHERE playlist_id = ?1 ORDER BY position",
            )?;
            let mut ids: Vec<String> = stmt
                .query_map([playlist_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ids.sort_by_key(|rid| rank(rid));
            ids
        };
        for (i, release_id) in ledger.iter().enumerate() {
            conn.execute(
                "UPDATE playlist_discovery_releases SET position = ?1, _hlc = ?2 WHERE playlist_id = ?3 AND release_id = ?4",
                rusqlite::params![i as i32, hlc, playlist_id, release_id],
            )?;
        }
        if !ledger.is_empty() {
            dirty::mark_dirty(&conn, buckets::PLAYLIST_DISCOVERY_RELEASES)?;
        }

        touch_playlist(&conn, playlist_id, &chrono::Utc::now().to_rfc3339(), &hlc)?;
        Ok(())
    }

    /// The playlist as release groups: each release carries only its member tracks (in
    /// album order) and `total_track_count`; groups are ordered by their first member's
    /// playlist position, followed by any pending trackless releases.
    pub fn get_playlist_releases(&self, playlist_id: &str) -> Result<Vec<DiscoveryRelease>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let members = load_members(&conn, playlist_id)?;
        let mut group_order: Vec<String> = Vec::new();
        let mut member_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (track_id, release_id, _) in &members {
            if !group_order.contains(release_id) {
                group_order.push(release_id.clone());
            }
            member_ids.insert(track_id.as_str());
        }
        {
            let mut stmt = conn.prepare(
                "SELECT release_id FROM playlist_discovery_releases WHERE playlist_id = ?1 ORDER BY position",
            )?;
            for rid in stmt.query_map([playlist_id], |row| row.get::<_, String>(0))? {
                let rid = rid?;
                if !group_order.contains(&rid) {
                    group_order.push(rid);
                }
            }
        }
        if group_order.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = group_order
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let param_refs: Vec<&dyn rusqlite::ToSql> = group_order
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                dr.id, dr.url, dr.source_type, dr.artist, dr.title, dr.label,
                dr.release_date, dr.artwork_url, dr.artwork_path,
                dr.notes, dr.parent_url, dr.source_page_url, dr.date_added, dr.date_modified
            FROM discovery_releases dr
            WHERE dr.id IN ({placeholders})"
        ))?;
        let mut by_id: std::collections::HashMap<String, DiscoveryRelease> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(DiscoveryRelease {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    source_type: row.get(2)?,
                    artist: row.get(3)?,
                    title: row.get(4)?,
                    label: row.get(5)?,
                    release_date: row.get(6)?,
                    artwork_url: row.get(7)?,
                    artwork_path: row.get(8)?,
                    artwork_cache_path: None,
                    notes: row.get(9)?,
                    parent_url: row.get(10)?,
                    source_page_url: row.get(11)?,
                    date_added: row.get(12)?,
                    date_modified: row.get(13)?,
                    is_new: false,
                    surfaced_at: None,
                    source_ids: Vec::new(),
                    tracks: Vec::new(),
                    tags: Vec::new(),
                    total_track_count: None,
                })
            })?
            .map(|r| r.map(|release| (release.id.clone(), release)))
            .collect::<std::result::Result<_, _>>()?;

        // Every track of every release, so the member filter and the total count come
        // from one read.
        let mut stmt = conn.prepare(&format!(
            "SELECT id, release_id, name, position, duration_ms, video_id, url, is_liked, liked_at,
                    EXISTS(SELECT 1 FROM discovery_preview_unavailable pu WHERE pu.release_id = discovery_tracks.release_id AND pu.position = discovery_tracks.position)
             FROM discovery_tracks WHERE release_id IN ({placeholders}) ORDER BY position"
        ))?;
        let mut all_tracks: Vec<DiscoveryTrack> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(DiscoveryTrack {
                    id: row.get(0)?,
                    release_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    duration_ms: row.get(4)?,
                    video_id: row.get(5)?,
                    url: row.get(6)?,
                    is_liked: row.get(7)?,
                    liked_at: row.get(8)?,
                    preview_unavailable: row.get::<_, i32>(9).map(|v| v != 0)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        crate::services::discovery::attach_track_tags(&conn, &mut all_tracks)?;

        let mut stmt = conn.prepare(&format!(
            "SELECT drt.release_id, t.id, t.category_id, t.name, t.color, t.sort_order
             FROM tags t
             INNER JOIN discovery_release_tags drt ON t.id = drt.tag_id
             WHERE drt.release_id IN ({placeholders})
             ORDER BY t.sort_order, t.name"
        ))?;
        let all_tags: Vec<(String, Tag)> = stmt
            .query_map(param_refs.as_slice(), |row| {
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
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut releases: Vec<DiscoveryRelease> = Vec::with_capacity(group_order.len());
        for release_id in &group_order {
            // A member whose release row is gone (mid-cascade) is simply not shown.
            let Some(mut release) = by_id.remove(release_id) else {
                continue;
            };
            let total = all_tracks
                .iter()
                .filter(|t| &t.release_id == release_id)
                .count();
            release.tracks = all_tracks
                .iter()
                .filter(|t| &t.release_id == release_id && member_ids.contains(t.id.as_str()))
                .cloned()
                .collect();
            release.total_track_count = Some(total as i32);
            release.tags = all_tags
                .iter()
                .filter(|(rid, _)| rid == release_id)
                .map(|(_, tag)| tag.clone())
                .collect();
            releases.push(release);
        }

        Ok(releases)
    }

    /// Fetch up to 4 distinct release covers for each of the given playlists, for mosaic
    /// thumbnails. Deliberately lightweight: no track/tag joins (unlike `get_playlist_releases`),
    /// so it can be batched across every playlist visible in a list. Dedupes by `artwork_url`
    /// (a playlist may hold releases that share a cover), ranks each distinct cover by its
    /// earliest member position, and keeps the first 4 per playlist. Returns an entry for every
    /// requested id (with an empty vec when a playlist has no usable covers) so callers can
    /// cache the "no covers" result and avoid refetching.
    pub fn get_playlist_cover_art(&self, playlist_ids: &[String]) -> Result<Vec<PlaylistCoverArt>> {
        // Pre-seed one entry per requested id, preserving input order.
        let mut result: Vec<PlaylistCoverArt> = playlist_ids
            .iter()
            .map(|id| PlaylistCoverArt {
                playlist_id: id.clone(),
                artwork_urls: Vec::new(),
            })
            .collect();

        if playlist_ids.is_empty() {
            return Ok(result);
        }

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let placeholders = playlist_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let param_refs: Vec<&dyn rusqlite::ToSql> = playlist_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        // Regular playlists: one batched window query over the member tracks' parent
        // releases. Smart playlists hold no junction rows (their members are computed
        // from rules), so they contribute nothing here and are resolved from their rules
        // below.
        {
            let sql = format!(
                r#"
                SELECT playlist_id, artwork_url FROM (
                    SELECT playlist_id, artwork_url,
                           ROW_NUMBER() OVER (PARTITION BY playlist_id ORDER BY first_pos) AS rn
                    FROM (
                        SELECT pdt.playlist_id AS playlist_id,
                               dr.artwork_url AS artwork_url,
                               MIN(pdt.position) AS first_pos
                        FROM playlist_discovery_tracks pdt
                        JOIN discovery_tracks dt ON dt.id = pdt.track_id
                        JOIN discovery_releases dr ON dr.id = dt.release_id
                        WHERE pdt.playlist_id IN ({placeholders})
                          AND dr.artwork_url IS NOT NULL AND dr.artwork_url <> ''
                        GROUP BY pdt.playlist_id, dr.artwork_url
                    )
                ) WHERE rn <= 4
                ORDER BY playlist_id, rn
                "#
            );

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<(String, String)>, _>>()?;

            for (playlist_id, artwork_url) in rows {
                if let Some(entry) = result.iter_mut().find(|c| c.playlist_id == playlist_id) {
                    entry.artwork_urls.push(artwork_url);
                }
            }
        }

        // Smart playlists: rule-based, so resolve each from its discovery query and keep the first four
        // distinct covers in the same order the detail view lists them (`get_smart_playlist_releases`). A
        // best-effort thumbnail fetch — a playlist with malformed rules is skipped (left coverless), not
        // allowed to fail the whole batch.
        let smart_meta: Vec<(String, Option<String>, String)> = {
            let mut stmt = conn.prepare(&format!(
                "SELECT id, smart_rules, context FROM playlists WHERE id IN ({placeholders}) AND is_smart = 1"
            ))?;
            // Bind before the block ends so the `MappedRows` temporary drops before `stmt` does (E0597).
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };

        for (playlist_id, smart_rules_json, context) in &smart_meta {
            // Covers come from `discovery_releases`, so only discovery smart playlists can be resolved here.
            if context != "discovery" {
                continue;
            }
            let Some(json) = smart_rules_json else {
                continue;
            };
            let rules: SmartRules = match serde_json::from_str(json) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let (where_clause, params) = match smart_rules::build_smart_query_discovery(&rules) {
                Ok(q) => q,
                Err(_) => continue,
            };

            let sql =
                format!("SELECT dr.artwork_url FROM discovery_releases dr WHERE {where_clause}");
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

            // Iterate lazily and stop at four distinct covers — for a sorted/limited smart query SQLite can
            // then avoid materializing the whole match set just to fill a thumbnail.
            let mut stmt = conn.prepare(&sql)?;
            let mut sql_rows = stmt.query(param_refs.as_slice())?;
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut urls: Vec<String> = Vec::new();
            while let Some(row) = sql_rows.next()? {
                let Some(url) = row.get::<_, Option<String>>(0)? else {
                    continue;
                };
                if url.is_empty() || !seen.insert(url.clone()) {
                    continue;
                }
                urls.push(url);
                if urls.len() >= 4 {
                    break;
                }
            }

            if let Some(entry) = result.iter_mut().find(|c| &c.playlist_id == playlist_id) {
                entry.artwork_urls = urls;
            }
        }

        Ok(result)
    }
}

fn load_members(conn: &Connection, playlist_id: &str) -> Result<Vec<MemberRow>> {
    let mut stmt = conn.prepare(
        "SELECT pdt.track_id, dt.release_id, pdt.position \
         FROM playlist_discovery_tracks pdt \
         JOIN discovery_tracks dt ON dt.id = pdt.track_id \
         WHERE pdt.playlist_id = ?1 \
         ORDER BY pdt.position, dt.position, dt.id",
    )?;
    let rows = stmt
        .query_map([playlist_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Append `track_ids` after the playlist's last member. Existing members are left
/// untouched (`OR IGNORE`), so re-adding a release only fills in its missing tracks.
fn insert_members(
    conn: &Connection,
    playlist_id: &str,
    track_ids: &[String],
    now: &str,
    hlc: &str,
) -> Result<()> {
    if track_ids.is_empty() {
        return Ok(());
    }
    let max_position: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) FROM playlist_discovery_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get(0),
        )
        .unwrap_or(-1);
    for (i, track_id) in track_ids.iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO playlist_discovery_tracks (playlist_id, track_id, position, date_added, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![playlist_id, track_id, max_position + 1 + i as i32, now, hlc],
        )?;
    }
    dirty::mark_dirty(conn, buckets::PLAYLIST_DISCOVERY_TRACKS)?;
    Ok(())
}

fn delete_members(
    conn: &Connection,
    playlist_id: &str,
    track_ids: &[String],
    hlc: &str,
) -> Result<()> {
    for track_id in track_ids {
        let deleted = conn.execute(
            "DELETE FROM playlist_discovery_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            rusqlite::params![playlist_id, track_id],
        )?;
        if deleted > 0 {
            dirty::record_tombstone(
                conn,
                buckets::PLAYLIST_DISCOVERY_TRACKS,
                &dirty::junction_entity_id(playlist_id, track_id),
                hlc,
            )?;
        }
    }
    dirty::mark_dirty(conn, buckets::PLAYLIST_DISCOVERY_TRACKS)?;
    Ok(())
}

/// Close the gaps a removal leaves so positions stay dense (the same rule the
/// release-level junction always followed).
fn compact_positions(conn: &Connection, playlist_id: &str, hlc: &str) -> Result<()> {
    let remaining: Vec<(String, i32)> = {
        let mut stmt = conn.prepare(
            "SELECT track_id, position FROM playlist_discovery_tracks WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let rows = stmt
            .query_map([playlist_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    for (i, (track_id, position)) in remaining.iter().enumerate() {
        if *position == i as i32 {
            continue;
        }
        conn.execute(
            "UPDATE playlist_discovery_tracks SET position = ?1, _hlc = ?2 WHERE playlist_id = ?3 AND track_id = ?4",
            rusqlite::params![i as i32, hlc, playlist_id, track_id],
        )?;
    }
    Ok(())
}

fn touch_playlist(conn: &Connection, playlist_id: &str, now: &str, hlc: &str) -> Result<()> {
    conn.execute(
        "UPDATE playlists SET date_modified = ?1, _hlc = ?2 WHERE id = ?3",
        rusqlite::params![now, hlc, playlist_id],
    )?;
    dirty::mark_dirty(conn, buckets::PLAYLISTS)?;
    Ok(())
}
