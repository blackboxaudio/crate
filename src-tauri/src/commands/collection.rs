//! Tauri command handlers for the purchased-collection feature — thin wrappers over
//! `CollectionService` and the `scrape` sync logic.

use tauri::{AppHandle, State};

use crate::error::{CrateError, Result};
use crate::models::{
    AccountRefreshResult, CollectionAccount, CollectionItem, CollectionOwnership,
    CollectionRefreshSummary,
};
use crate::services::collection::{scrape, watch};
use crate::services::discovery::metadata::{bandcamp_fan, build_client};
use crate::services::discovery::normalize_url;
use crate::services::CollectionService;

/// Link a pasted Bandcamp fan-page URL: fetch the page inline to validate it (wrong
/// URLs and private/empty collections fail here, keeping the link dialog open), create
/// the account with its profile, seed the first item batch, and walk the rest of the
/// collection in the background.
#[tauri::command]
pub async fn link_collection_account(
    url: String,
    app: AppHandle,
    collection: State<'_, CollectionService>,
) -> Result<CollectionAccount> {
    let normalized = normalize_url(&url);
    if !bandcamp_fan::is_bandcamp_fan_url(&normalized) {
        return Err(CrateError::Discovery(
            "Not a Bandcamp fan page URL (expected bandcamp.com/username)".into(),
        ));
    }

    let client = build_client()?;
    let page = bandcamp_fan::fetch_fan_page(&client, &normalized).await?;

    let account = collection.link_account(
        &normalized,
        "bandcamp",
        Some(&page.profile.fan_id.to_string()),
        page.profile.username.as_deref(),
        page.profile.name.as_deref(),
        page.profile.avatar_url.as_deref(),
    )?;

    // Seed what the page already gave us so the collection isn't empty while the
    // background walk runs (idempotent — the walk re-covers these items).
    collection.upsert_items(&account.id, &page.items)?;

    let conn = collection.connection();
    let app_data_dir = collection.app_data_dir();
    let id = account.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = scrape::sync_account(conn, app, app_data_dir, id, false).await {
            log::warn!("Initial collection sync failed: {e}");
        }
    });

    collection.get_account(&account.id)
}

#[tauri::command]
pub async fn unlink_collection_account(
    id: String,
    app: AppHandle,
    collection: State<'_, CollectionService>,
) -> Result<()> {
    collection.unlink_account(&id)?;
    let _ = tauri::Emitter::emit(&app, "collection-changed", &id);
    Ok(())
}

#[tauri::command]
pub async fn set_collection_account_enabled(
    id: String,
    enabled: bool,
    app: AppHandle,
    collection: State<'_, CollectionService>,
) -> Result<CollectionAccount> {
    let account = collection.set_enabled(&id, enabled)?;
    // Enabling/pausing changes which items count as owned everywhere.
    let _ = tauri::Emitter::emit(&app, "collection-changed", &id);
    Ok(account)
}

#[tauri::command]
pub async fn get_collection_accounts(
    collection: State<'_, CollectionService>,
) -> Result<Vec<CollectionAccount>> {
    collection.list_accounts()
}

#[tauri::command]
pub async fn get_collection_items(
    collection: State<'_, CollectionService>,
) -> Result<Vec<CollectionItem>> {
    collection.list_items()
}

/// Derived ownership id-sets for badge/filter rendering (see
/// `CollectionService::compute_ownership`).
#[tauri::command]
pub async fn get_collection_ownership(
    collection: State<'_, CollectionService>,
) -> Result<CollectionOwnership> {
    collection.compute_ownership()
}

/// "Purchased but not in library" cross-reference (fuzzy artist+album/title match
/// against the track library). Desktop-only — mobile has no library.
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn get_collection_library_gap(
    collection: State<'_, CollectionService>,
) -> Result<Vec<crate::models::CollectionGapItem>> {
    collection.library_gap()
}

/// Manual single-account "Refresh now": incremental walk (stops at already-known
/// items), so a steady-state refresh is one page fetch. Force past the auto-refresh
/// cooldown (the user asked for it), but the 429 backoff is still honored.
#[tauri::command]
pub async fn refresh_collection_account(
    id: String,
    app: AppHandle,
    collection: State<'_, CollectionService>,
) -> Result<AccountRefreshResult> {
    let account = collection.account_to_refresh(&id)?;
    let conn = collection.connection();
    let app_data_dir = collection.app_data_dir();
    Ok(watch::refresh_one(conn, app, app_data_dir, account, true).await)
}

/// Refresh every enabled account sequentially, spaced with jittered delays (all
/// accounts live on bandcamp.com). Forced — same gating rules as the single refresh.
#[tauri::command]
pub async fn refresh_all_collection_accounts(
    app: AppHandle,
    collection: State<'_, CollectionService>,
) -> Result<CollectionRefreshSummary> {
    let conn = collection.connection();
    let app_data_dir = collection.app_data_dir();
    watch::refresh_all(conn, app, app_data_dir, true).await
}
