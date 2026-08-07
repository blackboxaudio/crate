//! Ownership derivation: which discovery releases/tracks the user's collection
//! covers. Computed on read, never stored — new purchases, new releases, sync
//! merges, and account enable/disable are all reflected on the next call with
//! nothing to invalidate.
//!
//! Matching is URL identity. `collection_items.url` and `discovery_releases.url`
//! are both stored normalized, so release-level matching is a SQL join;
//! `discovery_tracks.url` is stored RAW (and NULL for rows predating migration
//! 10), so track-level matching normalizes in Rust.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::CollectionService;
use crate::error::{CrateError, Result};
#[cfg(feature = "desktop")]
use crate::models::CollectionGapItem;
use crate::models::CollectionOwnership;
use crate::services::discovery::normalize_url;

/// Fuzzy text normalization for matching purchases against library tracks: trim,
/// Unicode-lowercase, drop bracketed segments (`(Original Mix)`, `[Remastered]`),
/// cut `feat.`/`ft.` suffixes, collapse whitespace. Only the desktop-only library-gap
/// cross-reference needs it — mobile matches on URL identity alone.
#[cfg(feature = "desktop")]
fn normalize_for_match(s: &str) -> String {
    let lower = s.trim().to_lowercase();
    let mut cleaned = String::with_capacity(lower.len());
    let mut depth = 0u32;
    for c in lower.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => cleaned.push(c),
            _ => {}
        }
    }
    let cleaned = cleaned
        .split(" feat. ")
        .next()
        .unwrap_or(&cleaned)
        .split(" ft. ")
        .next()
        .unwrap_or(&cleaned)
        .to_string();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `(track_id, release_id, normalized_track_url)` for every discovery track that has
/// a page URL of its own.
fn tracks_with_urls(conn: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut stmt =
        conn.prepare("SELECT id, release_id, url FROM discovery_tracks WHERE url IS NOT NULL")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, release_id, url) = row?;
        out.push((id, release_id, normalize_url(&url)));
    }
    Ok(out)
}

/// Owned item URLs across enabled accounts (already normalized at write time),
/// split by item type.
fn owned_urls(conn: &Connection) -> Result<(HashSet<String>, HashSet<String>)> {
    let mut stmt = conn.prepare(
        "SELECT ci.item_type, ci.url FROM collection_items ci \
         JOIN collection_accounts ca ON ca.id = ci.account_id AND ca.enabled = 1",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut albums = HashSet::new();
    let mut tracks = HashSet::new();
    for row in rows {
        let (item_type, url) = row?;
        if item_type == "track" {
            tracks.insert(url);
        } else {
            albums.insert(url);
        }
    }
    Ok((albums, tracks))
}

/// Normalized track URL → parent release id, for matching track purchases to the
/// release that contains them (used by `list_items` to make purchases tappable).
pub(super) fn track_url_release_map(conn: &Connection) -> Result<HashMap<String, String>> {
    Ok(tracks_with_urls(conn)?
        .into_iter()
        .map(|(_, release_id, url)| (url, release_id))
        .collect())
}

impl CollectionService {
    /// Derive ownership of every local discovery release/track from the enabled
    /// accounts' items. A release is FULLY owned when an owned item's URL equals the
    /// release URL (album purchase — or a track purchase for a single-track release),
    /// or when every one of its URL-bearing tracks is individually owned; PARTIALLY
    /// owned when at least one (but not all) of its tracks is owned.
    pub fn compute_ownership(&self) -> Result<CollectionOwnership> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let (owned_album_urls, owned_track_urls) = owned_urls(&conn)?;

        // Release-level: item URL == release URL (any item type).
        let mut fully: HashSet<String> = {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT dr.id FROM discovery_releases dr \
                 JOIN collection_items ci ON ci.url = dr.url \
                 JOIN collection_accounts ca ON ca.id = ci.account_id AND ca.enabled = 1",
            )?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<std::result::Result<HashSet<_>, _>>()?
        };

        // Track-level: normalize each discovery track's URL and look it up.
        let mut owned_track_ids = Vec::new();
        let mut owned_count: HashMap<String, usize> = HashMap::new();
        let mut url_track_count: HashMap<String, usize> = HashMap::new();
        for (track_id, release_id, url) in tracks_with_urls(&conn)? {
            *url_track_count.entry(release_id.clone()).or_default() += 1;
            if owned_track_urls.contains(&url) || owned_album_urls.contains(&url) {
                owned_track_ids.push(track_id);
                *owned_count.entry(release_id).or_default() += 1;
            }
        }

        let mut partially: Vec<String> = Vec::new();
        for (release_id, owned) in owned_count {
            if fully.contains(&release_id) {
                continue;
            }
            // Promote to fully-owned only when every URL-bearing track is owned AND
            // the release has no URL-less tracks (unknown coverage stays partial).
            let with_urls = url_track_count.get(&release_id).copied().unwrap_or(0);
            let total = conn.query_row(
                "SELECT COUNT(*) FROM discovery_tracks WHERE release_id = ?1",
                [&release_id],
                |r| r.get::<_, i64>(0),
            )? as usize;
            if owned == total && with_urls == total {
                fully.insert(release_id);
            } else {
                partially.push(release_id);
            }
        }

        let mut fully: Vec<String> = fully.into_iter().collect();
        fully.sort();
        partially.sort();
        owned_track_ids.sort();
        Ok(CollectionOwnership {
            fully_owned_release_ids: fully,
            partially_owned_release_ids: partially,
            owned_track_ids,
        })
    }
}

#[cfg(feature = "desktop")]
impl CollectionService {
    /// Cross-reference every owned item against the local track library by normalized
    /// artist + album (album purchases) or artist + title (track purchases). Computed
    /// on demand — no `tracks` columns, nothing persisted. Desktop-only in practice
    /// (mobile's `tracks` table is always empty).
    pub fn library_gap(&self) -> Result<Vec<CollectionGapItem>> {
        let items = self.list_items()?;

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let mut albums: HashSet<(String, String)> = HashSet::new();
        let mut titles: HashSet<(String, String)> = HashSet::new();
        let mut stmt = conn.prepare("SELECT artist, album, title FROM tracks")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (artist, album, title) = row?;
            let artist = normalize_for_match(artist.as_deref().unwrap_or(""));
            if artist.is_empty() {
                continue;
            }
            if let Some(album) = album.as_deref().map(normalize_for_match) {
                if !album.is_empty() {
                    albums.insert((artist.clone(), album));
                }
            }
            if let Some(title) = title.as_deref().map(normalize_for_match) {
                if !title.is_empty() {
                    titles.insert((artist.clone(), title));
                }
            }
        }
        drop(stmt);
        drop(conn);

        Ok(items
            .into_iter()
            .map(|item| {
                let artist = normalize_for_match(item.artist.as_deref().unwrap_or(""));
                let title = normalize_for_match(item.title.as_deref().unwrap_or(""));
                let key = (artist, title);
                let in_library = !key.0.is_empty()
                    && !key.1.is_empty()
                    && if item.item_type == "track" {
                        titles.contains(&key)
                    } else {
                        albums.contains(&key)
                    };
                CollectionGapItem { item, in_library }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use rusqlite::params;

    use super::*;
    use crate::db::schema::get_migrations;

    fn service() -> CollectionService {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        for sql in get_migrations() {
            conn.execute_batch(sql).unwrap();
        }
        CollectionService::new(Arc::new(Mutex::new(conn)), PathBuf::new())
    }

    fn seed_account(svc: &CollectionService, id: &str, enabled: bool) {
        let conn = svc.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO collection_accounts (id, url, enabled, date_added, date_modified) \
             VALUES (?1, ?1, ?2, '2020-01-01', '2020-01-01')",
            params![id, enabled],
        )
        .unwrap();
    }

    fn seed_item(svc: &CollectionService, account: &str, item_type: &str, url: &str) {
        let conn = svc.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO collection_items (id, account_id, item_type, url, date_added, date_modified) \
             VALUES (?1, ?2, ?3, ?4, '2020-01-01', '2020-01-01')",
            params![format!("{account}|{url}"), account, item_type, url],
        )
        .unwrap();
    }

    fn seed_release(svc: &CollectionService, id: &str, url: &str, track_urls: &[Option<&str>]) {
        let conn = svc.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO discovery_releases (id, url, date_added, date_modified) \
             VALUES (?1, ?2, '2020-01-01', '2020-01-01')",
            params![id, url],
        )
        .unwrap();
        for (i, track_url) in track_urls.iter().enumerate() {
            conn.execute(
                "INSERT INTO discovery_tracks (id, release_id, name, position, url) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("{id}-t{i}"),
                    id,
                    format!("t{i}"),
                    i as i32,
                    track_url
                ],
            )
            .unwrap();
        }
    }

    #[test]
    fn album_purchase_fully_owns_matching_release() {
        let svc = service();
        seed_account(&svc, "acct", true);
        seed_item(&svc, "acct", "album", "https://a.bandcamp.com/album/x");
        seed_release(
            &svc,
            "rel",
            "https://a.bandcamp.com/album/x",
            &[Some("https://a.bandcamp.com/track/one")],
        );
        seed_release(&svc, "other", "https://b.bandcamp.com/album/y", &[]);

        let o = svc.compute_ownership().unwrap();
        assert_eq!(o.fully_owned_release_ids, vec!["rel"]);
        assert!(o.partially_owned_release_ids.is_empty());
        assert!(o.owned_track_ids.is_empty());
    }

    #[test]
    fn track_purchases_mark_partial_then_promote_to_full() {
        let svc = service();
        seed_account(&svc, "acct", true);
        seed_release(
            &svc,
            "rel",
            "https://a.bandcamp.com/album/x",
            &[
                Some("https://a.bandcamp.com/track/one"),
                Some("https://a.bandcamp.com/track/two"),
            ],
        );

        seed_item(&svc, "acct", "track", "https://a.bandcamp.com/track/one");
        let o = svc.compute_ownership().unwrap();
        assert!(o.fully_owned_release_ids.is_empty());
        assert_eq!(o.partially_owned_release_ids, vec!["rel"]);
        assert_eq!(o.owned_track_ids, vec!["rel-t0"]);

        seed_item(&svc, "acct", "track", "https://a.bandcamp.com/track/two");
        let o = svc.compute_ownership().unwrap();
        assert_eq!(o.fully_owned_release_ids, vec!["rel"]);
        assert!(o.partially_owned_release_ids.is_empty());
        assert_eq!(o.owned_track_ids, vec!["rel-t0", "rel-t1"]);
    }

    #[test]
    fn urlless_tracks_block_full_promotion() {
        let svc = service();
        seed_account(&svc, "acct", true);
        // Two tracks, one with no URL (pre-migration-10 row): owning the URL-bearing
        // one must NOT promote to fully owned — coverage of the other is unknown.
        seed_release(
            &svc,
            "rel",
            "https://a.bandcamp.com/album/x",
            &[Some("https://a.bandcamp.com/track/one"), None],
        );
        seed_item(&svc, "acct", "track", "https://a.bandcamp.com/track/one");

        let o = svc.compute_ownership().unwrap();
        assert!(o.fully_owned_release_ids.is_empty());
        assert_eq!(o.partially_owned_release_ids, vec!["rel"]);
    }

    #[test]
    fn disabled_accounts_do_not_count() {
        let svc = service();
        seed_account(&svc, "acct", false);
        seed_item(&svc, "acct", "album", "https://a.bandcamp.com/album/x");
        seed_release(&svc, "rel", "https://a.bandcamp.com/album/x", &[]);

        let o = svc.compute_ownership().unwrap();
        assert!(o.fully_owned_release_ids.is_empty());
        assert!(o.partially_owned_release_ids.is_empty());
        assert!(o.owned_track_ids.is_empty());
    }

    #[test]
    fn raw_track_urls_are_normalized_before_matching() {
        let svc = service();
        seed_account(&svc, "acct", true);
        // discovery_tracks.url is stored raw — trailing slash + tracking params must
        // still match the normalized item URL.
        seed_release(
            &svc,
            "rel",
            "https://a.bandcamp.com/album/x",
            &[
                Some("https://a.bandcamp.com/track/one/?utm_source=share"),
                Some("https://a.bandcamp.com/track/two"),
            ],
        );
        seed_item(&svc, "acct", "track", "https://a.bandcamp.com/track/one");

        let o = svc.compute_ownership().unwrap();
        assert_eq!(o.owned_track_ids, vec!["rel-t0"]);
        assert_eq!(o.partially_owned_release_ids, vec!["rel"]);
    }
}
