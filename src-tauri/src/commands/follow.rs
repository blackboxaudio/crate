//! Tauri command handlers for the follow feature — thin wrappers over `FollowService`
//! and the `watch` check logic.

use tauri::{AppHandle, State};

use crate::error::{CrateError, Result};
use crate::models::{
    FollowType, FollowedReleasesFound, FollowedSource, FollowedSourceCreate, SourceCheckResult,
};
use crate::services::discovery::{followable_page_url, metadata, normalize_url};
use crate::services::follow::watch;
use crate::services::FollowService;

/// Follow a pasted artist/label page URL: scan it to detect platform + artist-vs-label
/// and name, create the follow, and record the forward-looking baseline inline so the
/// modal can confirm success (or surface an unsupported-URL error).
#[tauri::command]
pub async fn follow_source(
    url: String,
    app: AppHandle,
    follow: State<'_, FollowService>,
) -> Result<FollowedSource> {
    let normalized = normalize_url(&url);
    let existing = std::collections::HashSet::new();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let page = metadata::scan_page(&normalized, &existing, &cancel, Some(&app)).await?;

    // Only Discogs pages reliably distinguish artist vs label; Bandcamp can't, so default
    // to artist (the page is watched correctly either way).
    let follow_type =
        if page.source_type == "discogs" && page.page_label.is_some() && page.page_artist.is_none()
        {
            "label"
        } else {
            "artist"
        };
    let name = page.page_artist.clone().or_else(|| page.page_label.clone());
    // Prefer the page's profile picture; fall back to the first release's artwork.
    let artwork_url = page
        .avatar_url
        .clone()
        .or_else(|| page.releases.first().and_then(|r| r.artwork_url.clone()));

    // Store the canonical followable page (Bandcamp origin, SoundCloud profile) so the
    // stored follow URL matches releases' `source_page_url` (and dedupes consistently),
    // regardless of the exact page URL the user pasted (e.g. a trailing `/music`).
    let created = follow.create_follow(FollowedSourceCreate {
        url: followable_page_url(&normalized, &page.source_type).unwrap_or(normalized),
        source_type: Some(page.source_type.clone()),
        follow_type: Some(follow_type.to_string()),
        name,
        artwork_url,
    })?;

    let urls: Vec<String> = page.releases.iter().map(|r| r.url.clone()).collect();
    follow.record_baseline(&created.id, &urls)?;
    follow.get_follow(&created.id)
}

/// Follow a known entity (the row popover): the caller supplies name/type/platform/url
/// from a release, so the follow is created immediately and the baseline is scanned in
/// the background (keeping the popover toggle instant).
#[tauri::command]
pub async fn follow_from_entity(
    create: FollowedSourceCreate,
    app: AppHandle,
    follow: State<'_, FollowService>,
) -> Result<FollowedSource> {
    let created = follow.create_follow(create)?;

    let conn = follow.connection();
    let app_data_dir = follow.app_data_dir();
    let id = created.id.clone();
    let url = created.url.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = watch::establish_baseline(conn, app, app_data_dir, id, url).await {
            log::warn!("follow baseline scan failed: {e}");
        }
    });

    Ok(created)
}

#[tauri::command]
pub async fn unfollow_source(id: String, follow: State<'_, FollowService>) -> Result<()> {
    follow.unfollow(&id)
}

/// Re-link a followed source to existing Discovery releases (bandaid for libraries
/// imported before `source_page_url` existed). Scans the source's page and stamps
/// `source_page_url` onto matching releases. Returns the number of releases linked.
#[tauri::command]
pub async fn relink_followed_source(
    id: String,
    app: AppHandle,
    follow: State<'_, FollowService>,
) -> Result<usize> {
    let conn = follow.connection();
    let app_data_dir = follow.app_data_dir();
    watch::relink_source(conn, app, app_data_dir, id).await
}

#[tauri::command]
pub async fn set_follow_enabled(
    id: String,
    enabled: bool,
    follow: State<'_, FollowService>,
) -> Result<FollowedSource> {
    follow.set_enabled(&id, enabled)
}

/// Correct a follow's artist-vs-label classification after the fact (the Following row's
/// type toggle), avoiding a destructive unfollow + re-follow.
#[tauri::command]
pub async fn set_follow_type(
    id: String,
    follow_type: String,
    follow: State<'_, FollowService>,
) -> Result<FollowedSource> {
    // Validate + normalize (e.g. "Artist" → "artist") against the known variants.
    let parsed: FollowType = follow_type.parse().map_err(CrateError::Discovery)?;
    follow.set_follow_type(&id, &parsed.to_string())
}

#[tauri::command]
pub async fn get_followed_sources(follow: State<'_, FollowService>) -> Result<Vec<FollowedSource>> {
    follow.list_follows()
}

#[tauri::command]
pub async fn check_followed_source(
    id: String,
    app: AppHandle,
    follow: State<'_, FollowService>,
) -> Result<SourceCheckResult> {
    let source = follow.source_to_check(&id)?;
    let conn = follow.connection();
    let app_data_dir = follow.app_data_dir();
    let (result, _ids) = watch::check_one(conn, app, app_data_dir, source).await;
    Ok(result)
}

#[tauri::command]
pub async fn check_all_followed_sources(
    app: AppHandle,
    follow: State<'_, FollowService>,
) -> Result<FollowedReleasesFound> {
    let conn = follow.connection();
    let app_data_dir = follow.app_data_dir();
    watch::check_all(conn, app, app_data_dir).await
}

/// Manual "mark as new / not-new" override (the automatic clear-on-action rule lives in
/// the discovery commands).
#[tauri::command]
pub async fn set_release_new_flag(
    release_id: String,
    is_new: bool,
    follow: State<'_, FollowService>,
) -> Result<()> {
    follow.set_new_flag(&release_id, is_new)
}
