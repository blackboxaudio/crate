//! Ownership derivation: which discovery releases/tracks the user's collection
//! covers. Computed on read, never stored — new purchases, new releases, sync
//! merges, and account enable/disable are all reflected on the next call with
//! nothing to invalidate.
//!
//! Matching is URL identity first: `collection_items.url` and
//! `discovery_releases.url` are both stored normalized, so release-level matching
//! is a SQL join; `discovery_tracks.url` is stored RAW (and NULL for rows
//! predating migration 10), so track-level matching normalizes in Rust. Because
//! most stored tracks predate per-track URLs (they backfill lazily on metadata
//! fetch), a track purchase falls back to name identity scoped to the release's
//! page host: same host + same normalized title. Both sides of that key come from
//! the same Bandcamp page, so exact normalized equality is safe.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::CollectionService;
use crate::error::{CrateError, Result};
#[cfg(feature = "desktop")]
use crate::models::CollectionGapItem;
use crate::models::{normalized_track_name, CollectionOwnership};
use crate::services::discovery::normalize_url;

/// Lowercased authority (host) of a URL, the scope for name-identity fallback
/// matching. `None` for scheme-less strings.
fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let end = rest.find('/').unwrap_or(rest.len());
    let host = &rest[..end];
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

/// Fuzzy text normalization for matching purchases against library tracks: trim,
/// Unicode-lowercase, drop bracketed segments (`(Original Mix)`, `[Remastered]`),
/// cut `feat.`/`ft.` suffixes, collapse whitespace. Only the desktop-only library-gap
/// cross-reference needs this fuzziness — ownership matching uses URL identity and
/// the exact host+title fallback.
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

/// One discovery track with everything both matching paths need: its own
/// normalized page URL (when stored) and the parent release's host + its
/// normalized name for the fallback key.
struct TrackRef {
    track_id: String,
    release_id: String,
    norm_url: Option<String>,
    release_host: Option<String>,
    norm_name: String,
}

/// Every discovery track joined to its release URL.
fn all_tracks(conn: &Connection) -> Result<Vec<TrackRef>> {
    let mut stmt = conn.prepare(
        "SELECT dt.id, dt.release_id, dt.name, dt.url, dr.url \
         FROM discovery_tracks dt JOIN discovery_releases dr ON dr.id = dt.release_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (track_id, release_id, name, url, release_url) = row?;
        out.push(TrackRef {
            track_id,
            release_id,
            norm_url: url.as_deref().map(normalize_url),
            release_host: url_host(&release_url),
            norm_name: normalized_track_name(&name),
        });
    }
    Ok(out)
}

/// Owned items across enabled accounts: URLs (already normalized at write time)
/// split by item type, plus the `(host, normalized title)` fallback keys of track
/// purchases.
struct OwnedItems {
    album_urls: HashSet<String>,
    track_urls: HashSet<String>,
    track_keys: HashSet<(String, String)>,
}

fn owned_items(conn: &Connection) -> Result<OwnedItems> {
    let mut stmt = conn.prepare(
        "SELECT ci.item_type, ci.url, ci.title FROM collection_items ci \
         JOIN collection_accounts ca ON ca.id = ci.account_id AND ca.enabled = 1",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut owned = OwnedItems {
        album_urls: HashSet::new(),
        track_urls: HashSet::new(),
        track_keys: HashSet::new(),
    };
    for row in rows {
        let (item_type, url, title) = row?;
        if item_type == "track" {
            if let (Some(host), Some(title)) = (url_host(&url), title) {
                let norm_title = normalized_track_name(&title);
                if !norm_title.is_empty() {
                    owned.track_keys.insert((host, norm_title));
                }
            }
            owned.track_urls.insert(url);
        } else {
            owned.album_urls.insert(url);
        }
    }
    Ok(owned)
}

/// Resolves a track purchase to the discovery release containing that track
/// (used by `list_items` to make purchases tappable): track-URL identity first,
/// then the host+title fallback. Collisions (the same track on several releases)
/// resolve to the smallest release id so every device picks the same one.
pub(super) struct PurchaseMatcher {
    by_url: HashMap<String, String>,
    by_host_title: HashMap<(String, String), String>,
}

impl PurchaseMatcher {
    pub(super) fn build(conn: &Connection) -> Result<Self> {
        let mut by_url: HashMap<String, String> = HashMap::new();
        let mut by_host_title: HashMap<(String, String), String> = HashMap::new();
        for t in all_tracks(conn)? {
            if let Some(u) = &t.norm_url {
                by_url
                    .entry(u.clone())
                    .and_modify(|r| {
                        if t.release_id < *r {
                            *r = t.release_id.clone();
                        }
                    })
                    .or_insert_with(|| t.release_id.clone());
            }
            if let (Some(host), false) = (&t.release_host, t.norm_name.is_empty()) {
                by_host_title
                    .entry((host.clone(), t.norm_name.clone()))
                    .and_modify(|r| {
                        if t.release_id < *r {
                            *r = t.release_id.clone();
                        }
                    })
                    .or_insert_with(|| t.release_id.clone());
            }
        }
        Ok(Self {
            by_url,
            by_host_title,
        })
    }

    /// `title` should only be passed for track-type purchases — an album title
    /// must never name-match a track.
    pub(super) fn release_for(&self, url: &str, title: Option<&str>) -> Option<String> {
        if let Some(release_id) = self.by_url.get(url) {
            return Some(release_id.clone());
        }
        let norm_title = normalized_track_name(title?);
        if norm_title.is_empty() {
            return None;
        }
        self.by_host_title
            .get(&(url_host(url)?, norm_title))
            .cloned()
    }
}

impl CollectionService {
    /// Derive ownership of every local discovery release/track from the enabled
    /// accounts' items. A release is FULLY owned when an owned item's URL equals the
    /// release URL (album purchase — or a track purchase for a single-track release),
    /// or when every one of its tracks is individually owned; PARTIALLY owned when
    /// at least one (but not all) of its tracks is owned. A track is owned by URL
    /// identity or by the host+title fallback.
    pub fn compute_ownership(&self) -> Result<CollectionOwnership> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let owned = owned_items(&conn)?;

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

        // Track-level: URL identity when the track has a page URL, else the
        // host+title fallback.
        let mut owned_track_ids = Vec::new();
        let mut owned_count: HashMap<String, usize> = HashMap::new();
        let mut track_count: HashMap<String, usize> = HashMap::new();
        for t in all_tracks(&conn)? {
            *track_count.entry(t.release_id.clone()).or_default() += 1;
            let by_url = t
                .norm_url
                .as_ref()
                .is_some_and(|u| owned.track_urls.contains(u) || owned.album_urls.contains(u));
            let by_title = !t.norm_name.is_empty()
                && t.release_host
                    .as_ref()
                    .is_some_and(|h| owned.track_keys.contains(&(h.clone(), t.norm_name.clone())));
            if by_url || by_title {
                owned_track_ids.push(t.track_id);
                *owned_count.entry(t.release_id).or_default() += 1;
            }
        }

        let mut partially: Vec<String> = Vec::new();
        for (release_id, owned_n) in owned_count {
            if fully.contains(&release_id) {
                continue;
            }
            let total = track_count.get(&release_id).copied().unwrap_or(0);
            if owned_n == total {
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
        // Two tracks, one with no URL (pre-migration-10 row) and no matching title:
        // owning the URL-bearing one must NOT promote to fully owned.
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

    fn seed_track_item(svc: &CollectionService, account: &str, url: &str, title: &str) {
        let conn = svc.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO collection_items (id, account_id, item_type, url, title, date_added, date_modified) \
             VALUES (?1, ?2, 'track', ?3, ?4, '2020-01-01', '2020-01-01')",
            params![format!("{account}|{url}"), account, url, title],
        )
        .unwrap();
    }

    #[test]
    fn urlless_track_owned_via_host_and_title_fallback() {
        let svc = service();
        seed_account(&svc, "acct", true);
        // Tracks are seeded as "t0"/"t1" with no page URLs — the 96%-of-rows case.
        seed_release(&svc, "rel", "https://a.bandcamp.com/album/x", &[None, None]);
        // Purchase URL shares the release's host; title matches modulo trim/case.
        seed_track_item(&svc, "acct", "https://a.bandcamp.com/track/whatever", " T0 ");

        let o = svc.compute_ownership().unwrap();
        assert_eq!(o.owned_track_ids, vec!["rel-t0"]);
        assert_eq!(o.partially_owned_release_ids, vec!["rel"]);
        assert!(o.fully_owned_release_ids.is_empty());
    }

    #[test]
    fn title_fallback_promotes_to_full_when_every_track_owned() {
        let svc = service();
        seed_account(&svc, "acct", true);
        seed_release(&svc, "rel", "https://a.bandcamp.com/album/x", &[None, None]);
        seed_track_item(&svc, "acct", "https://a.bandcamp.com/track/one", "t0");
        seed_track_item(&svc, "acct", "https://a.bandcamp.com/track/two", "t1");

        let o = svc.compute_ownership().unwrap();
        assert_eq!(o.fully_owned_release_ids, vec!["rel"]);
        assert!(o.partially_owned_release_ids.is_empty());
        assert_eq!(o.owned_track_ids, vec!["rel-t0", "rel-t1"]);
    }

    #[test]
    fn title_fallback_is_scoped_to_the_release_host() {
        let svc = service();
        seed_account(&svc, "acct", true);
        seed_release(&svc, "rel", "https://a.bandcamp.com/album/x", &[None]);
        // Same title, different subdomain: must not match.
        seed_track_item(&svc, "acct", "https://b.bandcamp.com/track/whatever", "t0");

        let o = svc.compute_ownership().unwrap();
        assert!(o.owned_track_ids.is_empty());
        assert!(o.partially_owned_release_ids.is_empty());
        assert!(o.fully_owned_release_ids.is_empty());
    }

    #[test]
    fn purchase_matcher_resolves_release_by_url_then_title() {
        let svc = service();
        seed_account(&svc, "acct", true);
        seed_release(
            &svc,
            "rel-a",
            "https://a.bandcamp.com/album/x",
            &[Some("https://a.bandcamp.com/track/one"), None],
        );
        let conn = svc.conn.lock().unwrap();
        let matcher = PurchaseMatcher::build(&conn).unwrap();
        assert_eq!(
            matcher.release_for("https://a.bandcamp.com/track/one", None),
            Some("rel-a".to_string()),
            "track URL identity needs no title"
        );
        assert_eq!(
            matcher.release_for("https://a.bandcamp.com/track/unknown", Some("T1")),
            Some("rel-a".to_string()),
            "host+title fallback finds the containing release"
        );
        assert_eq!(
            matcher.release_for("https://b.bandcamp.com/track/unknown", Some("t1")),
            None,
            "fallback stays scoped to the host"
        );
        assert_eq!(
            matcher.release_for("https://a.bandcamp.com/track/unknown", None),
            None,
            "no title, no fallback"
        );
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
