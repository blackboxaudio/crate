//! Synchronous DB methods for `CollectionService`. Synced tables
//! (`collection_accounts`, `collection_items`) stamp `_hlc` + mark the bucket dirty;
//! the per-device `collection_account_state` does neither.

use std::collections::HashSet;

use rusqlite::OptionalExtension;

use super::{AccountToRefresh, CollectionService};
use crate::error::{CrateError, Result};
use crate::models::{
    deterministic_account_id, deterministic_collection_item_id, CollectionAccount, CollectionItem,
    FollowHealth, ScrapedCollectionItem,
};
use crate::services::cloud_sync::pipeline::{buckets, dirty};
use crate::services::discovery::normalize_url;
use crate::services::watch_gate::CheckGate;

/// Full SELECT for a `CollectionAccount`: the synced row joined to local refresh state,
/// with the item count computed inline.
const ACCOUNT_SELECT: &str = "SELECT \
    ca.id, ca.url, ca.source_type, ca.external_id, ca.username, ca.name, ca.avatar_url, \
    ca.enabled, ca.date_added, ca.date_modified, \
    st.last_checked_at, COALESCE(st.health, 'unknown'), st.last_error, \
    (SELECT COUNT(*) FROM collection_items ci WHERE ci.account_id = ca.id) \
    FROM collection_accounts ca \
    LEFT JOIN collection_account_state st ON st.account_id = ca.id";

/// SELECT for the Purchased view: items of enabled accounts, each LEFT-JOINed to the
/// discovery release with the identical (normalized) URL, newest purchases first.
const ITEM_SELECT: &str = "SELECT \
    ci.id, ci.account_id, ci.source_type, ci.item_type, ci.url, ci.external_id, \
    ci.artist, ci.title, ci.artwork_url, ci.purchased_at, ci.date_added, dr.id \
    FROM collection_items ci \
    JOIN collection_accounts ca ON ca.id = ci.account_id AND ca.enabled = 1 \
    LEFT JOIN discovery_releases dr ON dr.url = ci.url \
    ORDER BY ci.purchased_at IS NULL, ci.purchased_at DESC, ci.date_added DESC";

fn map_account_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionAccount> {
    Ok(CollectionAccount {
        id: row.get(0)?,
        url: row.get(1)?,
        source_type: row.get(2)?,
        external_id: row.get(3)?,
        username: row.get(4)?,
        name: row.get(5)?,
        avatar_url: row.get(6)?,
        enabled: row.get(7)?,
        date_added: row.get(8)?,
        date_modified: row.get(9)?,
        last_checked_at: row.get(10)?,
        health: row.get(11)?,
        last_error: row.get(12)?,
        item_count: row.get(13)?,
    })
}

fn map_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionItem> {
    Ok(CollectionItem {
        id: row.get(0)?,
        account_id: row.get(1)?,
        source_type: row.get(2)?,
        item_type: row.get(3)?,
        url: row.get(4)?,
        external_id: row.get(5)?,
        artist: row.get(6)?,
        title: row.get(7)?,
        artwork_url: row.get(8)?,
        purchased_at: row.get(9)?,
        date_added: row.get(10)?,
        matched_release_id: row.get(11)?,
    })
}

impl CollectionService {
    /// Insert a collection-account row + its local state row. Idempotent on URL (the
    /// deterministic id encodes it): re-linking re-enables and refreshes the profile.
    pub fn link_account(
        &self,
        url: &str,
        source_type: &str,
        external_id: Option<&str>,
        username: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<CollectionAccount> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let normalized_url = normalize_url(url);
        let id = deterministic_account_id(&normalized_url);

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "INSERT INTO collection_accounts \
                (id, url, source_type, external_id, username, name, avatar_url, enabled, date_added, date_modified, _hlc) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8, ?9) \
             ON CONFLICT(id) DO UPDATE SET enabled = 1, external_id = COALESCE(?4, external_id), \
                username = COALESCE(?5, username), name = COALESCE(?6, name), \
                avatar_url = COALESCE(?7, avatar_url), date_modified = ?8, _hlc = ?9",
            rusqlite::params![
                id,
                normalized_url,
                source_type,
                external_id,
                username,
                name,
                avatar_url,
                now,
                hlc
            ],
        )?;
        dirty::mark_dirty(&conn, buckets::COLLECTION_ACCOUNTS)?;

        conn.execute(
            "INSERT OR IGNORE INTO collection_account_state (account_id) VALUES (?1)",
            [&id],
        )?;

        drop(conn);
        self.get_account(&id)
    }

    pub fn get_account(&self, id: &str) -> Result<CollectionAccount> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let sql = format!("{ACCOUNT_SELECT} WHERE ca.id = ?1");
        conn.query_row(&sql, [id], map_account_row)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CrateError::Discovery(format!("Collection account not found: {id}"))
                }
                _ => CrateError::Database(e),
            })
    }

    pub fn list_accounts(&self) -> Result<Vec<CollectionAccount>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let sql = format!("{ACCOUNT_SELECT} ORDER BY ca.date_added");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_account_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Unlink an account: tombstone every item plus the account (all under one HLC —
    /// one logical mutation), then delete; the cascade removes items + state locally
    /// and peers converge through the same cascade when the account delete merges.
    /// Items still need their own tombstones because a peer may hold item rows this
    /// device never saw (partial scrape) — the account cascade covers those too, but
    /// only if the peer processes the account delete; explicit item tombstones make
    /// the outcome independent of merge order.
    pub fn unlink_account(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let hlc = dirty::next_hlc(&conn)?;

        let item_ids: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM collection_items WHERE account_id = ?1")?;
            let rows = stmt.query_map([id], |r| r.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        for item_id in &item_ids {
            dirty::record_tombstone(&conn, buckets::COLLECTION_ITEMS, item_id, &hlc)?;
        }
        dirty::record_tombstone(&conn, buckets::COLLECTION_ACCOUNTS, id, &hlc)?;

        conn.execute("DELETE FROM collection_accounts WHERE id = ?1", [id])?;
        dirty::mark_dirty(&conn, buckets::COLLECTION_ACCOUNTS)?;
        dirty::mark_dirty(&conn, buckets::COLLECTION_ITEMS)?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<CollectionAccount> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE collection_accounts SET enabled = ?1, date_modified = ?2, _hlc = ?3 WHERE id = ?4",
            rusqlite::params![enabled, now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, buckets::COLLECTION_ACCOUNTS)?;
        drop(conn);
        self.get_account(id)
    }

    /// Backfill the account's profile (fan_id, handle, display name, avatar) from a
    /// completed page fetch.
    pub fn set_profile(
        &self,
        id: &str,
        external_id: Option<&str>,
        username: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        let changed = conn.execute(
            "UPDATE collection_accounts SET external_id = COALESCE(?1, external_id), \
             username = COALESCE(?2, username), name = COALESCE(?3, name), \
             avatar_url = COALESCE(?4, avatar_url), date_modified = ?5, _hlc = ?6 \
             WHERE id = ?7 AND (COALESCE(?1, '') != COALESCE(external_id, '') \
                OR COALESCE(?2, '') != COALESCE(username, '') \
                OR COALESCE(?3, '') != COALESCE(name, '') \
                OR COALESCE(?4, '') != COALESCE(avatar_url, ''))",
            rusqlite::params![external_id, username, name, avatar_url, now, hlc, id],
        )?;
        if changed > 0 {
            dirty::mark_dirty(&conn, buckets::COLLECTION_ACCOUNTS)?;
        }
        Ok(())
    }

    /// Upsert a scraped batch. One HLC per batch (one logical mutation); rows whose
    /// content is unchanged are left untouched so steady-state rescans don't churn
    /// `_hlc` (and re-upload the bucket) for nothing. Returns how many were NEW.
    pub fn upsert_items(&self, account_id: &str, items: &[ScrapedCollectionItem]) -> Result<usize> {
        if items.is_empty() {
            return Ok(0);
        }
        let known = self.known_item_ids(account_id)?;

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;
        let mut new_count = 0usize;
        let mut changed_any = false;

        for item in items {
            let normalized_url = normalize_url(&item.url);
            let id = deterministic_collection_item_id(account_id, &normalized_url);
            if !known.contains(&id) {
                new_count += 1;
            }
            let changed = conn.execute(
                "INSERT INTO collection_items \
                    (id, account_id, source_type, item_type, url, external_id, artist, title, \
                     artwork_url, purchased_at, date_added, date_modified, _hlc) \
                 VALUES (?1, ?2, 'bandcamp', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?11) \
                 ON CONFLICT(id) DO UPDATE SET item_type = ?3, external_id = ?5, artist = ?6, \
                    title = ?7, artwork_url = ?8, purchased_at = COALESCE(?9, purchased_at), \
                    date_modified = ?10, _hlc = ?11 \
                 WHERE item_type != ?3 OR COALESCE(external_id, '') != COALESCE(?5, '') \
                    OR COALESCE(artist, '') != COALESCE(?6, '') \
                    OR COALESCE(title, '') != COALESCE(?7, '') \
                    OR COALESCE(artwork_url, '') != COALESCE(?8, '') \
                    OR COALESCE(?9, COALESCE(purchased_at, '')) != COALESCE(purchased_at, '')",
                rusqlite::params![
                    id,
                    account_id,
                    item.item_type,
                    normalized_url,
                    item.external_id,
                    item.artist,
                    item.title,
                    item.artwork_url,
                    item.purchased_at,
                    now,
                    hlc
                ],
            )?;
            changed_any |= changed > 0;
        }
        if changed_any {
            dirty::mark_dirty(&conn, buckets::COLLECTION_ITEMS)?;
        }
        Ok(new_count)
    }

    /// Deterministic ids of every item already stored for an account — the incremental
    /// scrape's stop set.
    pub fn known_item_ids(&self, account_id: &str) -> Result<HashSet<String>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let mut stmt = conn.prepare("SELECT id FROM collection_items WHERE account_id = ?1")?;
        let rows = stmt.query_map([account_id], |r| r.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    }

    /// The union of owned items across enabled accounts, newest purchases first,
    /// deduped by URL (the same release owned on two accounts appears once). A track
    /// purchase whose URL matches no release directly is matched to the discovery
    /// release *containing* that track — by track URL, else by host+title — so it
    /// still opens the release detail.
    pub fn list_items(&self) -> Result<Vec<CollectionItem>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let matcher = super::matching::PurchaseMatcher::build(&conn)?;
        let mut stmt = conn.prepare(ITEM_SELECT)?;
        let rows = stmt.query_map([], map_item_row)?;
        let mut seen_urls = HashSet::new();
        let mut items = Vec::new();
        for row in rows {
            let mut item = row?;
            if item.matched_release_id.is_none() {
                // Title fallback only for track purchases: an album title must
                // never name-match a track.
                let title = (item.item_type == "track")
                    .then_some(item.title.as_deref())
                    .flatten();
                item.matched_release_id = matcher.release_for(&item.url, title);
            }
            if seen_urls.insert(item.url.clone()) {
                items.push(item);
            }
        }
        Ok(items)
    }

    pub fn mark_checked(
        &self,
        account_id: &str,
        health: FollowHealth,
        error: Option<&str>,
        item_count: Option<i64>,
        walk_complete: Option<bool>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        let health_str = health.to_string();
        if matches!(health, FollowHealth::Ok) {
            conn.execute(
                "UPDATE collection_account_state SET last_checked_at = ?2, last_success_at = ?2, \
                 health = ?3, last_error = NULL, consecutive_failures = 0, \
                 last_item_count = COALESCE(?4, last_item_count), \
                 last_walk_complete = COALESCE(?5, last_walk_complete) WHERE account_id = ?1",
                rusqlite::params![account_id, now, health_str, item_count, walk_complete],
            )?;
        } else {
            conn.execute(
                "UPDATE collection_account_state SET last_checked_at = ?2, health = ?3, \
                 last_error = ?4, consecutive_failures = consecutive_failures + 1 WHERE account_id = ?1",
                rusqlite::params![account_id, now, health_str, error],
            )?;
        }
        Ok(())
    }

    /// Whether this account's last collection walk covered everything — the
    /// precondition for the incremental scrape's stop-on-all-known early exit.
    /// A missing state row reads as `false` (never fully walked).
    pub fn last_walk_complete(&self, account_id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let complete: Option<bool> = conn
            .query_row(
                "SELECT last_walk_complete != 0 FROM collection_account_state WHERE account_id = ?1",
                [account_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(complete.unwrap_or(false))
    }

    /// Local refresh-state fields for the check gate (cooldown + failure backoff).
    pub fn get_check_gate(&self, account_id: &str) -> Result<CheckGate> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        conn.query_row(
            "SELECT last_checked_at, COALESCE(health, 'unknown'), last_error, consecutive_failures \
             FROM collection_account_state WHERE account_id = ?1",
            [account_id],
            |r| {
                Ok(CheckGate {
                    last_checked_at: r.get(0)?,
                    health: r.get(1)?,
                    last_error: r.get(2)?,
                    consecutive_failures: r.get(3)?,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn enabled_accounts(&self) -> Result<Vec<AccountToRefresh>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let mut stmt = conn.prepare(
            "SELECT id, url, name FROM collection_accounts WHERE enabled = 1 ORDER BY date_added",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AccountToRefresh {
                id: r.get(0)?,
                url: r.get(1)?,
                name: r.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn account_to_refresh(&self, id: &str) -> Result<AccountToRefresh> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        conn.query_row(
            "SELECT id, url, name FROM collection_accounts WHERE id = ?1",
            [id],
            |r| {
                Ok(AccountToRefresh {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    name: r.get(2)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CrateError::Discovery(format!("Collection account not found: {id}"))
            }
            _ => CrateError::Database(e),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use rusqlite::Connection;

    use super::*;

    fn service() -> CollectionService {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        for sql in crate::db::schema::get_migrations() {
            conn.execute_batch(sql).unwrap();
        }
        CollectionService::new(Arc::new(Mutex::new(conn)), PathBuf::new())
    }

    #[test]
    fn walk_completeness_round_trips_through_account_state() {
        let svc = service();
        let account = svc
            .link_account(
                "https://bandcamp.com/fan",
                "bandcamp",
                None,
                None,
                None,
                None,
            )
            .unwrap();

        // Never walked: the incremental early exit must not be trusted.
        assert!(!svc.last_walk_complete(&account.id).unwrap());

        svc.mark_checked(&account.id, FollowHealth::Ok, None, Some(10), Some(true))
            .unwrap();
        assert!(svc.last_walk_complete(&account.id).unwrap());

        // A failed check leaves coverage untouched…
        svc.mark_checked(&account.id, FollowHealth::Error, Some("boom"), None, None)
            .unwrap();
        assert!(svc.last_walk_complete(&account.id).unwrap());

        // …and so does an OK check that doesn't report on it.
        svc.mark_checked(&account.id, FollowHealth::Ok, None, None, None)
            .unwrap();
        assert!(svc.last_walk_complete(&account.id).unwrap());

        // A capped walk (MAX_BATCHES) demotes coverage until a full walk finishes.
        svc.mark_checked(&account.id, FollowHealth::Ok, None, None, Some(false))
            .unwrap();
        assert!(!svc.last_walk_complete(&account.id).unwrap());
    }
}
