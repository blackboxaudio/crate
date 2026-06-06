//! The follow watch loop's per-source check logic, shared by the manual "Check now"
//! commands and (later) the background timer. Functions are module-level `async fn`
//! taking the connection Arc and constructing the services they need, so the DB lock
//! is never held across the network `scan_page` await.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::error::Result;
use crate::models::{
    DiscoveryReleaseCreate, FollowHealth, FollowedReleasesFound, ScannedPage, SourceCheckResult,
};
use crate::services::discovery::{metadata, DiscoveryService};

use super::{diff, FollowService, SourceToCheck};

/// Scan a page for the forward-looking baseline: the full current page contents,
/// independent of what is already in Discovery.
async fn scan_for_baseline(url: &str, app: &AppHandle) -> Result<ScannedPage> {
    let existing = HashSet::new();
    let cancel = AtomicBool::new(false);
    metadata::scan_page(url, &existing, &cancel, Some(app)).await
}

/// Establish a source's baseline (record the current page as "known", surface nothing)
/// and flip `baseline_established`. Used for sources without a local baseline yet — a
/// brand-new follow, or one synced in from another device.
pub async fn establish_baseline(
    conn: Arc<Mutex<Connection>>,
    app: AppHandle,
    app_data_dir: PathBuf,
    source_id: String,
    url: String,
) -> Result<()> {
    let follow = FollowService::new(conn, app_data_dir);
    match scan_for_baseline(&url, &app).await {
        Ok(page) => {
            let urls: Vec<String> = page.releases.into_iter().map(|r| r.url).collect();
            follow.record_baseline(&source_id, &urls)
        }
        Err(e) => {
            let _ = follow.mark_checked(&source_id, FollowHealth::Error, Some(&e.to_string()));
            Err(e)
        }
    }
}

/// Check one source. If it has no baseline yet, establish it (surfacing nothing).
/// Otherwise surface every release new since the baseline: create it flagged "new" if
/// it isn't already in Discovery, else just attach this source's provenance. Returns
/// the per-source result plus the ids of any newly-created releases.
pub async fn check_one(
    conn: Arc<Mutex<Connection>>,
    app: AppHandle,
    app_data_dir: PathBuf,
    source: SourceToCheck,
) -> (SourceCheckResult, Vec<String>) {
    let follow = FollowService::new(conn.clone(), app_data_dir.clone());
    let name = source.name.clone();

    // Baseline pass: a source with no local baseline (new follow, or synced from another
    // device) records the page as known and surfaces nothing — this is the anti-flood guard.
    if !source.baseline_established {
        let res = establish_baseline(
            conn.clone(),
            app.clone(),
            app_data_dir.clone(),
            source.id.clone(),
            source.url.clone(),
        )
        .await;
        let (health, error) = match res {
            Ok(()) => ("ok".to_string(), None),
            Err(e) => ("error".to_string(), Some(e.to_string())),
        };
        return (
            SourceCheckResult {
                source_id: source.id,
                name,
                new_count: 0,
                health,
                error,
            },
            Vec::new(),
        );
    }

    let page = match scan_for_baseline(&source.url, &app).await {
        Ok(p) => p,
        Err(e) => {
            let _ = follow.mark_checked(&source.id, FollowHealth::Error, Some(&e.to_string()));
            return (
                SourceCheckResult {
                    source_id: source.id,
                    name,
                    new_count: 0,
                    health: "error".to_string(),
                    error: Some(e.to_string()),
                },
                Vec::new(),
            );
        }
    };

    let seen = follow.get_seen_urls(&source.id).unwrap_or_default();
    let new_releases = diff::compute_new_urls(&page.releases, &seen);

    let discovery = DiscoveryService::new(conn.clone(), app_data_dir.clone());
    let mut new_count = 0usize;
    let mut release_ids = Vec::new();

    for scanned in new_releases {
        let url = scanned.url.clone(); // already normalized by scan_page
        let release_id = match follow.release_id_for_url(&url).unwrap_or(None) {
            // Already in Discovery (manually added, or surfaced by another follow this
            // sweep): dedup — attach provenance below, don't duplicate or re-flag "new".
            Some(rid) => rid,
            None => {
                let create = DiscoveryReleaseCreate {
                    url: url.clone(),
                    source_type: Some(source.source_type.clone()),
                    artist: scanned.artist.clone(),
                    title: scanned.title.clone(),
                    label: None,
                    release_date: scanned.release_date.clone(),
                    artwork_url: scanned.artwork_url.clone(),
                    notes: None,
                    parent_url: None,
                    tracks: None,
                };
                match discovery.create_release(create) {
                    Ok(rel) => {
                        let _ = follow.mark_surfaced(&rel.id);
                        new_count += 1;
                        release_ids.push(rel.id.clone());
                        rel.id
                    }
                    Err(e) => {
                        log::warn!("follow: failed to surface {url}: {e}");
                        continue;
                    }
                }
            }
        };
        let _ = follow.add_provenance(&release_id, &source.id);
        let _ = follow.record_seen(&source.id, &url, "surfaced", Some(&release_id));
    }

    let _ = follow.mark_checked(&source.id, FollowHealth::Ok, None);
    (
        SourceCheckResult {
            source_id: source.id,
            name,
            new_count,
            health: "ok".to_string(),
            error: None,
        },
        release_ids,
    )
}

/// Check every enabled source sequentially (rate-limit friendly, with per-platform
/// spacing — Discogs especially), aggregate, and emit `followed-releases-found`.
pub async fn check_all(
    conn: Arc<Mutex<Connection>>,
    app: AppHandle,
    app_data_dir: PathBuf,
) -> Result<FollowedReleasesFound> {
    let follow = FollowService::new(conn.clone(), app_data_dir.clone());
    let sources = follow.enabled_sources()?;

    let mut by_source = Vec::new();
    let mut release_ids = Vec::new();
    for source in sources {
        let base_ms = match source.source_type.as_str() {
            "discogs" => 8000,
            "soundcloud" => 2000,
            _ => 1500,
        };
        tokio::time::sleep(metadata::jittered_delay(base_ms)).await;

        let (result, ids) =
            check_one(conn.clone(), app.clone(), app_data_dir.clone(), source).await;
        release_ids.extend(ids);
        by_source.push(result);
    }

    let total_new: usize = by_source.iter().map(|r| r.new_count).sum();
    let found = FollowedReleasesFound {
        total_new,
        by_source,
        release_ids,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("followed-releases-found", &found);
    Ok(found)
}
