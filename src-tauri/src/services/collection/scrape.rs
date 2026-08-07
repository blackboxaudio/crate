//! Async fan-page collection scraping: the initial page fetch plus the paginated
//! collection-API walk that fills `collection_items`.
//!
//! Items arrive newest-first, so an *incremental* sync stops as soon as an entire
//! batch is already known — a steady-state refresh is one page fetch and usually zero
//! API calls. Items that later disappear from the collection (refunds, newly hidden)
//! are NOT pruned; Bandcamp collections effectively only grow.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::Serialize;
use tauri::Emitter;

use super::CollectionService;
use crate::error::Result;
use crate::models::{deterministic_collection_item_id, FollowHealth, ScrapedCollectionItem};
use crate::services::discovery::metadata::{bandcamp_fan, build_client, jittered_delay};
use crate::services::discovery::normalize_url;

/// Validated `count` for the collection API (it honors at least 100 per request).
const COLLECTION_BATCH_SIZE: u32 = 100;
/// Runaway-pagination backstop (250 × 100 = a 25k-item collection).
const MAX_BATCHES: usize = 250;
/// Base delay between collection-API requests (jittered ±50%).
const BATCH_DELAY_MS: u64 = 1500;

pub struct ScrapeOutcome {
    pub new_items: usize,
    pub total_seen: usize,
    /// False only when the walk hit `MAX_BATCHES` with more still available.
    pub complete: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SyncProgress<'a> {
    account_id: &'a str,
    fetched: usize,
    total: Option<usize>,
}

fn health_for_error(msg: &str) -> FollowHealth {
    let lower = msg.to_lowercase();
    if lower.contains("rate limit") || lower.contains("429") || lower.contains("too many requests")
    {
        FollowHealth::RateLimited
    } else {
        FollowHealth::Error
    }
}

/// Sync one linked account's collection into `collection_items`. Holds no DB lock
/// across awaits. Updates the account's refresh state on both success and failure;
/// emits `collection-sync-progress` per batch and `collection-changed` on completion.
pub async fn sync_account(
    conn: Arc<Mutex<Connection>>,
    app: tauri::AppHandle,
    app_data_dir: PathBuf,
    account_id: String,
    incremental: bool,
) -> Result<ScrapeOutcome> {
    let service = CollectionService::new(conn, app_data_dir);
    let account = service.account_to_refresh(&account_id)?;

    let outcome = run_sync(&service, &app, &account_id, &account.url, incremental).await;
    match &outcome {
        Ok(o) => {
            let _ = app.emit("collection-changed", &account_id);
            log::info!(
                "Collection sync for {}: {} new of {} seen (complete: {})",
                account.name.as_deref().unwrap_or(&account.url),
                o.new_items,
                o.total_seen,
                o.complete
            );
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = service.mark_checked(&account_id, health_for_error(&msg), Some(&msg), None, None);
            log::warn!("Collection sync failed for {}: {msg}", account.url);
        }
    }
    outcome
}

async fn run_sync(
    service: &CollectionService,
    app: &tauri::AppHandle,
    account_id: &str,
    url: &str,
    incremental: bool,
) -> Result<ScrapeOutcome> {
    // The stop-on-all-known early exit assumes stored items form a newest-first
    // PREFIX of the collection. An interrupted walk (network error, app suspended
    // mid-pagination) leaves the newest items known and older ones never fetched —
    // incremental syncs would then exit on page 1 forever. Only trust the invariant
    // when the last walk actually reached the end.
    let incremental = if incremental && !service.last_walk_complete(account_id)? {
        log::info!("Collection sync for {url}: previous walk incomplete — running a full walk");
        false
    } else {
        incremental
    };

    let client = build_client()?;
    let page = bandcamp_fan::fetch_fan_page(&client, url).await?;

    // Backfill profile identity (fan_id is required for pagination on later syncs).
    let fan_id = page.profile.fan_id;
    service.set_profile(
        account_id,
        Some(&fan_id.to_string()),
        page.profile.username.as_deref(),
        page.profile.name.as_deref(),
        page.profile.avatar_url.as_deref(),
    )?;

    // Snapshot of what was known BEFORE this sync: the incremental stop set. Items are
    // newest-first, so once an entire batch is in here, everything older is too.
    let known = if incremental {
        service.known_item_ids(account_id)?
    } else {
        Default::default()
    };
    let all_known = |items: &[ScrapedCollectionItem]| {
        incremental
            && !items.is_empty()
            && items.iter().all(|i| {
                known.contains(&deterministic_collection_item_id(
                    account_id,
                    &normalize_url(&i.url),
                ))
            })
    };

    let total = page.item_count;
    let mut new_items = service.upsert_items(account_id, &page.items)?;
    let mut total_seen = page.items.len();
    let mut token = page.last_token;
    let _ = app.emit(
        "collection-sync-progress",
        SyncProgress {
            account_id,
            fetched: total_seen,
            total,
        },
    );

    let mut complete = true;
    if !all_known(&page.items) {
        for batch_no in 0..MAX_BATCHES {
            let Some(older_than) = token.as_deref().filter(|t| !t.is_empty()) else {
                break;
            };
            if total.is_some_and(|t| total_seen >= t) {
                break;
            }
            // The page itself was request #1 against bandcamp.com; space out the rest.
            tokio::time::sleep(jittered_delay(BATCH_DELAY_MS)).await;

            let batch = bandcamp_fan::fetch_collection_batch(
                &client,
                fan_id,
                older_than,
                COLLECTION_BATCH_SIZE,
            )
            .await?;

            new_items += service.upsert_items(account_id, &batch.items)?;
            total_seen += batch.items.len();
            token = batch.last_token;
            let _ = app.emit(
                "collection-sync-progress",
                SyncProgress {
                    account_id,
                    fetched: total_seen,
                    total,
                },
            );

            if !batch.more_available || all_known(&batch.items) {
                break;
            }
            if batch_no + 1 == MAX_BATCHES {
                complete = false;
            }
        }
    }

    // `complete` doubles as walk coverage: a full walk that finishes reached the
    // end, and an incremental one only ran because coverage was already complete.
    service.mark_checked(
        account_id,
        FollowHealth::Ok,
        None,
        total.map(|t| t as i64),
        Some(complete),
    )?;
    Ok(ScrapeOutcome {
        new_items,
        total_seen,
        complete,
    })
}
