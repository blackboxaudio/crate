//! The collection refresh loop: per-account gated incremental syncs, shared by the
//! manual "Refresh" commands and the background timer. Mirrors `follow::watch`, minus
//! baselines/notifications — a refresh only upserts owned items.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use rusqlite::Connection;
use tauri::AppHandle;

use crate::error::Result;
use crate::models::{AccountRefreshResult, CollectionRefreshSummary, FollowHealth};
use crate::services::discovery::metadata;
use crate::services::follow::watch::cadence_schedule;
use crate::services::watch_gate::should_skip_scan;
use crate::services::SettingsService;

use super::{scrape, AccountToRefresh, CollectionService};

/// Don't re-scrape a collection more often than this on automatic sweeps. Much longer
/// than follow's page cooldown — purchases change rarely, and a steady-state incremental
/// refresh still costs one page fetch per account. Manual "Refresh now" bypasses it
/// (`force`); the shared failure backoff (`watch_gate`) is honored even then.
const RESCAN_COOLDOWN_SECS: i64 = 6 * 60 * 60;

/// At most one full refresh sweep at a time (launch sweep vs pull-to-refresh vs
/// "Refresh now" races — same rationale as the follow sweep lock).
static SWEEP_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Refresh one account, gated by the cooldown/backoff. A skipped account reports its
/// current health with zero new items.
pub async fn refresh_one(
    conn: Arc<Mutex<Connection>>,
    app: AppHandle,
    app_data_dir: PathBuf,
    account: AccountToRefresh,
    force: bool,
) -> AccountRefreshResult {
    let service = CollectionService::new(conn.clone(), app_data_dir.clone());

    let gate_says_skip = service
        .get_check_gate(&account.id)
        .map(|g| should_skip_scan(Utc::now(), &g, force, RESCAN_COOLDOWN_SECS))
        .unwrap_or(false);
    if gate_says_skip {
        let health = service
            .get_check_gate(&account.id)
            .map(|g| g.health)
            .unwrap_or_else(|_| "unknown".to_string());
        return AccountRefreshResult {
            account_id: account.id,
            name: account.name,
            new_items: 0,
            health,
            error: None,
        };
    }

    match scrape::sync_account(conn, app, app_data_dir, account.id.clone(), true).await {
        Ok(outcome) => AccountRefreshResult {
            account_id: account.id,
            name: account.name,
            new_items: outcome.new_items,
            health: FollowHealth::Ok.to_string(),
            error: None,
        },
        // sync_account already recorded the failure in the account's state.
        Err(e) => {
            let health = service
                .get_check_gate(&account.id)
                .map(|g| g.health)
                .unwrap_or_else(|_| FollowHealth::Error.to_string());
            AccountRefreshResult {
                account_id: account.id,
                name: account.name,
                new_items: 0,
                health,
                error: Some(e.to_string()),
            }
        }
    }
}

/// Refresh every enabled account sequentially, spaced with jittered delays (they all
/// live on bandcamp.com).
pub async fn refresh_all(
    conn: Arc<Mutex<Connection>>,
    app: AppHandle,
    app_data_dir: PathBuf,
    force: bool,
) -> Result<CollectionRefreshSummary> {
    let Ok(_sweep) = SWEEP_LOCK.try_lock() else {
        log::info!("collection: refresh already in progress — skipping duplicate sweep");
        return Ok(CollectionRefreshSummary {
            total_new: 0,
            by_account: Vec::new(),
            checked_at: Utc::now().to_rfc3339(),
        });
    };

    let service = CollectionService::new(conn.clone(), app_data_dir.clone());
    let accounts = service.enabled_accounts()?;

    let mut by_account = Vec::with_capacity(accounts.len());
    let mut total_new = 0usize;
    for account in accounts {
        // Only pay the inter-account spacing when this account will actually be scraped.
        let will_scan = service
            .get_check_gate(&account.id)
            .map(|g| !should_skip_scan(Utc::now(), &g, force, RESCAN_COOLDOWN_SECS))
            .unwrap_or(true);
        if will_scan {
            tokio::time::sleep(metadata::jittered_delay(2000)).await;
        }

        let result = refresh_one(
            conn.clone(),
            app.clone(),
            app_data_dir.clone(),
            account,
            force,
        )
        .await;
        total_new += result.new_items;
        by_account.push(result);
    }

    Ok(CollectionRefreshSummary {
        total_new,
        by_account,
        checked_at: Utc::now().to_rfc3339(),
    })
}

/// Spawn the background refresh loop. Mirrors `follow::watch::start_watching`, offset so
/// the two loops' launch sweeps never stampede bandcamp.com together. Reads the cadence
/// from settings each iteration; does zero work (and zero network requests) when no
/// account is linked.
pub fn start_watching(app_handle: AppHandle, conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        // Let startup (and the follow loop's 30s launch sweep) settle first.
        tokio::time::sleep(Duration::from_secs(90)).await;
        let mut first = true;
        loop {
            let settings = SettingsService::new(conn.clone())
                .get_settings()
                .unwrap_or_default();
            let (run_on_launch, interval) = cadence_schedule(settings.collection_refresh_cadence);

            let should_check = if first {
                run_on_launch
            } else {
                interval.is_some()
            };
            if should_check {
                // Mobile: don't burn background CPU on an automatic sweep while the app is
                // backgrounded (same iOS kill-risk rationale as the follow loop). Bounded,
                // fail-open wait for the webview to be foregrounded.
                #[cfg(feature = "mobile")]
                {
                    use std::sync::atomic::Ordering;
                    use tauri::Manager;
                    let cap = Duration::from_secs(15 * 60);
                    let mut waited = Duration::ZERO;
                    while !app_handle
                        .state::<crate::AppForegroundFlag>()
                        .0
                        .load(Ordering::Relaxed)
                        && waited < cap
                    {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        waited += Duration::from_secs(30);
                    }
                }
                if let Err(e) =
                    refresh_all(conn.clone(), app_handle.clone(), app_data_dir.clone(), false).await
                {
                    log::warn!("collection: background refresh failed: {e}");
                }
            }
            first = false;

            // Cadences with no interval (On launch / Manual) still loop slowly so
            // settings changes apply without a restart.
            tokio::time::sleep(interval.unwrap_or(Duration::from_secs(3600))).await;
        }
    });
}
