//! Models for the purchased-collection feature: linked Bandcamp fan accounts and the
//! owned items scraped from them. `source_type` discriminates future purchase sources
//! (Beatport, …) even though v1 implements only Bandcamp.
//!
//! A `CollectionAccount` flattens the synced `collection_accounts` row together with
//! the device-local `collection_account_state` and an item count, mirroring
//! `FollowedSource`.

use serde::{Deserialize, Serialize};

/// Fixed namespace for content-derived collection-account ids (arbitrary; never change
/// it — every device must mint identical ids from identical inputs, forever).
pub const COLLECTION_ACCOUNT_ID_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x9a3f_7c1e_44b2_4d08_a5c6_e0d1_92b7_558a);

/// Fixed namespace for content-derived collection-item ids (same never-change rule).
pub const COLLECTION_ITEM_ID_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x3d84_c2f1_a967_4b5e_8f20_71bc_d4e9_066f);

/// Deterministic account id: UUIDv5 over the normalized fan-page URL (the account's
/// natural key — `collection_accounts.url` is UNIQUE). Two devices linking the same
/// account mint the same id, so cloud sync converges on one row.
pub fn deterministic_account_id(normalized_url: &str) -> String {
    uuid::Uuid::new_v5(&COLLECTION_ACCOUNT_ID_NAMESPACE, normalized_url.as_bytes()).to_string()
}

/// Deterministic item id: UUIDv5 over `account_id|normalized_item_url`. Encoding the
/// account means the same release owned on two linked accounts is two rows (unlinking
/// one account must not delete the other's copy), while two devices scraping the same
/// account converge by id instead of colliding.
pub fn deterministic_collection_item_id(account_id: &str, normalized_item_url: &str) -> String {
    uuid::Uuid::new_v5(
        &COLLECTION_ITEM_ID_NAMESPACE,
        format!("{account_id}|{normalized_item_url}").as_bytes(),
    )
    .to_string()
}

/// One owned item as produced by a source scraper (URL still raw — the service
/// normalizes at write time).
#[derive(Debug, Clone, Serialize)]
pub struct ScrapedCollectionItem {
    /// `"album"` or `"track"`.
    pub item_type: String,
    /// Raw item page URL — may be a `*.bandcamp.com` subdomain or a custom artist domain.
    pub url: String,
    /// Source-native numeric item id, kept for future id-based matching.
    pub external_id: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub artwork_url: Option<String>,
    /// RFC 3339 purchase timestamp when the source's date parses; `None` otherwise.
    pub purchased_at: Option<String>,
}

/// A linked collection account, flattened for the frontend: the synced row plus the
/// device-local refresh state and an item count.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionAccount {
    pub id: String,
    pub url: String,
    pub source_type: String,
    pub external_id: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub enabled: bool,
    pub date_added: String,
    pub date_modified: String,
    #[serde(default)]
    pub last_checked_at: Option<String>,
    #[serde(default)]
    pub health: String,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Owned items currently stored for this account.
    #[serde(default)]
    pub item_count: i64,
}

/// One owned item, for the Purchased view. `matched_release_id` is the discovery
/// release this purchase corresponds to (by URL identity), when one exists locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItem {
    pub id: String,
    pub account_id: String,
    pub source_type: String,
    pub item_type: String,
    pub url: String,
    pub external_id: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub artwork_url: Option<String>,
    pub purchased_at: Option<String>,
    pub date_added: String,
    #[serde(default)]
    pub matched_release_id: Option<String>,
}

/// Derived ownership of discovery releases/tracks, computed from `collection_items`
/// URLs at read time (never stored). `owned_track_ids` holds only INDIVIDUALLY
/// purchased tracks — a track inside a fully-owned release is implied by its release.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionOwnership {
    pub fully_owned_release_ids: Vec<String>,
    pub partially_owned_release_ids: Vec<String>,
    pub owned_track_ids: Vec<String>,
}

/// One purchase's presence in the local track library (the desktop "purchased but not
/// in library" gap view). Matching is fuzzy (normalized artist + album/title), so this
/// is advisory, never stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionGapItem {
    pub item: CollectionItem,
    pub in_library: bool,
}

/// Result of refreshing one linked account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRefreshResult {
    pub account_id: String,
    pub name: Option<String>,
    pub new_items: usize,
    pub health: String,
    pub error: Option<String>,
}

/// Aggregate payload returned by "refresh all collections".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRefreshSummary {
    pub total_new: usize,
    pub by_account: Vec<AccountRefreshResult>,
    pub checked_at: String,
}
