//! The follow watch loop's per-source check logic, shared by the manual "Check now"
//! commands and (later) the background timer. Functions are module-level `async fn`
//! taking the connection Arc and constructing the services they need, so the DB lock
//! is never held across the network `scan_page` await.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::error::Result;
use crate::models::{
    DiscoveryReleaseCreate, FollowCheckCadence, FollowHealth, FollowedReleasesFound, ScannedPage,
    SourceCheckResult,
};
use crate::services::discovery::{metadata, DiscoveryService};
use crate::services::SettingsService;

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

/// Spawn the background watch loop — a long-lived task that mirrors the device monitor.
/// The cadence is read from settings each iteration (so changes apply without a
/// restart). It does zero work, and makes zero network requests, when nothing is
/// followed (`check_all` early-returns over an empty enabled-source list).
pub fn start_watching(app_handle: AppHandle, conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        // Let startup settle before the first sweep.
        tokio::time::sleep(Duration::from_secs(30)).await;
        let mut first = true;
        loop {
            let settings = SettingsService::new(conn.clone())
                .get_settings()
                .unwrap_or_default();
            let (run_on_launch, interval) = cadence_schedule(settings.follow_check_cadence);

            // Release-day reminders fire independently of the check cadence (even Manual),
            // so an Upcoming release that hit its date while the app was closed still notifies.
            if settings.release_day_reminders {
                fire_release_day_notifications(conn.clone(), &app_handle, app_data_dir.clone())
                    .await;
            }

            let should_check = if first { run_on_launch } else { interval.is_some() };
            if should_check {
                if let Ok(found) =
                    check_all(conn.clone(), app_handle.clone(), app_data_dir.clone()).await
                {
                    // Foreground/background split: a backgrounded app gets a native
                    // summary notification; a focused app gets the in-app toast that the
                    // `followed-releases-found` event drives on the frontend.
                    if settings.new_releases_summary
                        && found.total_new > 0
                        && !window_focused(&app_handle)
                    {
                        fire_summary_notification(&app_handle, &found);
                    }
                }
            }
            first = false;

            // Sleep until the next sweep. Cadences with no interval (On launch / Manual)
            // still loop slowly so release-day reminders fire and settings changes apply.
            tokio::time::sleep(interval.unwrap_or(Duration::from_secs(3600))).await;
        }
    });
}

/// `(run_on_launch, periodic_interval)` for a cadence. "Daily" is on-launch + every 24h.
fn cadence_schedule(cadence: FollowCheckCadence) -> (bool, Option<Duration>) {
    match cadence {
        FollowCheckCadence::Manual => (false, None),
        FollowCheckCadence::OnLaunch => (true, None),
        FollowCheckCadence::Hourly => (true, Some(Duration::from_secs(3600))),
        FollowCheckCadence::Daily => (true, Some(Duration::from_secs(86_400))),
    }
}

fn window_focused(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false)
}

/// Fire one native notification per surfaced release that hits its release date today
/// and hasn't been announced yet (idempotent via `release_day_notified`).
async fn fire_release_day_notifications(
    conn: Arc<Mutex<Connection>>,
    app: &AppHandle,
    app_data_dir: PathBuf,
) {
    let follow = FollowService::new(conn, app_data_dir);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let due = match follow.releases_due_today(&today) {
        Ok(d) => d,
        Err(_) => return,
    };
    for item in due {
        let artist = item.artist.as_deref().unwrap_or("New release");
        let title = match &item.title {
            Some(t) => format!("{artist} — {t} is out today"),
            None => format!("{artist} is out today"),
        };
        let body = match &item.source_name {
            Some(n) => format!("From {n}, who you follow"),
            None => "From who you follow".to_string(),
        };
        let _ = app.notification().builder().title(title).body(body).show();
        let _ = follow.mark_release_day_notified(&item.release_id);
    }
}

fn fire_summary_notification(app: &AppHandle, found: &FollowedReleasesFound) {
    let n = found.total_new;
    let title = if n == 1 {
        "1 new release from who you follow".to_string()
    } else {
        format!("{n} new releases from who you follow")
    };
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body("Open Crate to review")
        .show();
}
