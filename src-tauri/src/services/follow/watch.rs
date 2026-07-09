//! The follow watch loop's per-source check logic, shared by the manual "Check now"
//! commands and (later) the background timer. Functions are module-level `async fn`
//! taking the connection Arc and constructing the services they need, so the DB lock
//! is never held across the network `scan_page` await.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
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

use super::{diff, CheckGate, FollowService, SourceToCheck};

/// Don't re-fetch a source's page more often than this on automatic sweeps — the cache
/// window that keeps a relaunch loop or a spammed "Check all" from hammering a platform
/// into a rate limit. A user's explicit single-source "Check now" bypasses it (`force`).
const RESCAN_COOLDOWN_SECS: i64 = 30 * 60;
/// Base backoff after a failure, doubled per consecutive failure (capped). Rate-limit
/// responses (HTTP 429) start longer than transient errors; both are honored even for a
/// forced manual check so a user can't hammer a source the platform is already throttling.
const BACKOFF_BASE_ERROR_SECS: i64 = 5 * 60;
const BACKOFF_BASE_RATE_LIMITED_SECS: i64 = 15 * 60;
const BACKOFF_MAX_SECS: i64 = 6 * 60 * 60;

/// Classify a scan error as a rate-limit (so the UI shows it and backoff runs longer) or
/// a generic failure. Bandcamp/SoundCloud/Discogs all surface 429s with a "rate limit"
/// / "429" marker in the message (see the metadata fetchers).
fn health_for_error(msg: &str) -> FollowHealth {
    let m = msg.to_lowercase();
    if m.contains("rate limit") || m.contains("429") || m.contains("too many requests") {
        FollowHealth::RateLimited
    } else {
        FollowHealth::Error
    }
}

/// Seconds elapsed since an RFC3339 timestamp, or `None` if it can't be parsed or is in
/// the future (a clock jump — treat as "unknown", i.e. don't skip).
fn seconds_since(now: DateTime<Utc>, ts: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|t| {
            now.signed_duration_since(t.with_timezone(&Utc))
                .num_seconds()
        })
        .filter(|&s| s >= 0)
}

/// The backoff window (seconds) implied by the last check's health + failure streak.
/// Zero when the last check succeeded (`consecutive_failures == 0`).
fn backoff_window_secs(health: &str, consecutive_failures: i64) -> i64 {
    if consecutive_failures < 1 {
        return 0;
    }
    let base = if health == "rate_limited" {
        BACKOFF_BASE_RATE_LIMITED_SECS
    } else {
        BACKOFF_BASE_ERROR_SECS
    };
    let shift = (consecutive_failures - 1).min(10) as u32;
    base.saturating_mul(1i64 << shift).min(BACKOFF_MAX_SECS)
}

/// Whether to skip a source's network scan right now. A failure backoff applies even to a
/// forced manual check; the plain re-scan cooldown applies only to automatic sweeps
/// (`force == false`). Returns `false` when there's no prior check to gate against.
fn should_skip_scan(now: DateTime<Utc>, gate: &CheckGate, force: bool) -> bool {
    let elapsed = match gate
        .last_checked_at
        .as_deref()
        .and_then(|ts| seconds_since(now, ts))
    {
        Some(e) => e,
        None => return false,
    };
    let backoff = backoff_window_secs(&gate.health, gate.consecutive_failures);
    if backoff > 0 && elapsed < backoff {
        return true;
    }
    !force && elapsed < RESCAN_COOLDOWN_SECS
}

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
            // Popover/import follows are created from a single release before the page is
            // fetched, so backfill from the now-scanned page. Fetch the source once.
            let source = follow.get_follow(&source_id).ok();
            // Profile picture — but never clobber an avatar already set.
            if let (Some(src), Some(avatar)) = (source.as_ref(), page.avatar_url.as_deref()) {
                if src.artwork_url.is_none() && src.artwork_path.is_none() {
                    let _ = follow.set_artwork(&source_id, Some(avatar));
                }
            }
            // Display name — the release-derived name can be the artist instead of the
            // label, or missing when metadata wasn't fetched. Prefer the page name that
            // matches the follow's type; only overwrite when it actually differs.
            if let Some(src) = source.as_ref() {
                let scanned_name = if src.follow_type == "label" {
                    page.page_label.as_deref().or(page.page_artist.as_deref())
                } else {
                    page.page_artist.as_deref().or(page.page_label.as_deref())
                };
                if let Some(name) = scanned_name {
                    if !name.is_empty() && src.name.as_deref() != Some(name) {
                        let _ = follow.set_name(&source_id, name);
                    }
                }
            }
            let urls: Vec<String> = page.releases.into_iter().map(|r| r.url).collect();
            follow.record_baseline(&source_id, &urls)
        }
        Err(e) => {
            let health = health_for_error(&e.to_string());
            let _ = follow.mark_checked(&source_id, health, Some(&e.to_string()));
            Err(e)
        }
    }
}

/// Re-link a followed source's page to existing Discovery releases — a bandaid for
/// libraries imported before `source_page_url` existed. Scans the page and stamps
/// `source_page_url` onto already-imported releases found on it (so the row's Label/Artist
/// follow state reflects this source), refreshes the follow's own name + avatar from the
/// page, and emits `discovery-release-updated` so the view patches live. Returns the
/// number of releases newly linked.
pub async fn relink_source(
    conn: Arc<Mutex<Connection>>,
    app: AppHandle,
    app_data_dir: PathBuf,
    source_id: String,
) -> Result<usize> {
    let follow = FollowService::new(conn.clone(), app_data_dir.clone());
    let discovery = DiscoveryService::new(conn, app_data_dir);
    let source = follow.get_follow(&source_id)?;

    let page = scan_for_baseline(&source.url, &app).await?;

    // Refresh the follow's name + avatar from the authoritative page (matched to type).
    let scanned_name = if source.follow_type == "label" {
        page.page_label.as_deref().or(page.page_artist.as_deref())
    } else {
        page.page_artist.as_deref().or(page.page_label.as_deref())
    };
    if let Some(name) = scanned_name {
        if !name.is_empty() && source.name.as_deref() != Some(name) {
            let _ = follow.set_name(&source_id, name);
        }
    }
    if let Some(avatar) = page.avatar_url.as_deref() {
        if source.artwork_url.is_none() && source.artwork_path.is_none() {
            let _ = follow.set_artwork(&source_id, Some(avatar));
        }
    }

    // Stamp the source page onto matching existing releases; emit each so rows refresh.
    let mut linked = 0usize;
    for scanned in &page.releases {
        match discovery.set_source_page_url_if_absent(&scanned.url, &source.url) {
            Ok(0) => {}
            Ok(_) => {
                linked += 1;
                if let Ok(Some(rid)) = follow.release_id_for_url(&scanned.url) {
                    if let Ok(updated) = discovery.get_release(&rid) {
                        let _ = app.emit("discovery-release-updated", &updated);
                    }
                }
            }
            Err(e) => log::warn!("relink: failed to link {}: {e}", scanned.url),
        }
    }

    let _ = follow.mark_checked(&source_id, FollowHealth::Ok, None);
    Ok(linked)
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
    force: bool,
) -> (SourceCheckResult, Vec<String>) {
    let follow = FollowService::new(conn.clone(), app_data_dir.clone());
    let name = source.name.clone();

    // Rate-limit gate: skip the network scan entirely if this source is inside its
    // failure-backoff window, or (on automatic sweeps) was scanned within the re-scan
    // cooldown. This is the cache that stops repeated sweeps from hammering
    // Bandcamp/SoundCloud/Discogs into a rate limit. A missing state row → never checked
    // → scan. Returns the last known health/error unchanged (no DB write, no clock reset).
    if let Ok(gate) = follow.get_check_gate(&source.id) {
        if should_skip_scan(Utc::now(), &gate, force) {
            return (
                SourceCheckResult {
                    source_id: source.id,
                    name,
                    new_count: 0,
                    health: gate.health,
                    error: gate.last_error,
                },
                Vec::new(),
            );
        }
    }

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
            Err(e) => (
                health_for_error(&e.to_string()).to_string(),
                Some(e.to_string()),
            ),
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
            let health = health_for_error(&e.to_string());
            let _ = follow.mark_checked(&source.id, health, Some(&e.to_string()));
            return (
                SourceCheckResult {
                    source_id: source.id,
                    name,
                    new_count: 0,
                    health: health.to_string(),
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
                    // Discovered via this followed source — stamp its page so the row
                    // reflects the follow and re-imports stay linked.
                    source_page_url: Some(source.url.clone()),
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
        // Only pay the inter-source rate-limit spacing when this source will actually be
        // scanned — a gated-out source makes no network call, so it needs no delay.
        let will_scan = follow
            .get_check_gate(&source.id)
            .map(|g| !should_skip_scan(Utc::now(), &g, false))
            .unwrap_or(true);
        if will_scan {
            let base_ms = match source.source_type.as_str() {
                "discogs" => 8000,
                "soundcloud" => 2000,
                _ => 1500,
            };
            tokio::time::sleep(metadata::jittered_delay(base_ms)).await;
        }

        let (result, ids) = check_one(
            conn.clone(),
            app.clone(),
            app_data_dir.clone(),
            source,
            false,
        )
        .await;
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

            let should_check = if first {
                run_on_launch
            } else {
                interval.is_some()
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ts: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(ts)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn gate(last_checked_at: Option<&str>, health: &str, failures: i64) -> CheckGate {
        CheckGate {
            last_checked_at: last_checked_at.map(|s| s.to_string()),
            health: health.to_string(),
            last_error: None,
            consecutive_failures: failures,
        }
    }

    #[test]
    fn never_checked_source_is_never_skipped() {
        let now = at("2026-07-05T12:00:00Z");
        assert!(!should_skip_scan(now, &gate(None, "unknown", 0), false));
        assert!(!should_skip_scan(now, &gate(None, "unknown", 0), true));
    }

    #[test]
    fn cooldown_skips_recent_ok_check_only_on_automatic_sweeps() {
        let now = at("2026-07-05T12:00:00Z");
        // Checked 10 min ago, healthy: within the 30-min cooldown.
        let recent = gate(Some("2026-07-05T11:50:00Z"), "ok", 0);
        assert!(should_skip_scan(now, &recent, false)); // automatic sweep: skip
        assert!(!should_skip_scan(now, &recent, true)); // forced manual: scan
    }

    #[test]
    fn cooldown_expires_after_the_window() {
        let now = at("2026-07-05T12:00:00Z");
        // Checked 31 min ago, healthy: past the cooldown.
        let old = gate(Some("2026-07-05T11:29:00Z"), "ok", 0);
        assert!(!should_skip_scan(now, &old, false));
    }

    #[test]
    fn rate_limit_backoff_is_honored_even_when_forced() {
        let now = at("2026-07-05T12:00:00Z");
        // Rate-limited 5 min ago, first failure → 15-min backoff window.
        let limited = gate(Some("2026-07-05T11:55:00Z"), "rate_limited", 1);
        assert!(should_skip_scan(now, &limited, false));
        assert!(should_skip_scan(now, &limited, true)); // forced still backs off
    }

    #[test]
    fn backoff_grows_with_consecutive_failures_and_caps() {
        // error base 5m: failure #1 → 5m, #2 → 10m, #3 → 20m.
        assert_eq!(backoff_window_secs("error", 1), 5 * 60);
        assert_eq!(backoff_window_secs("error", 2), 10 * 60);
        assert_eq!(backoff_window_secs("error", 3), 20 * 60);
        // rate_limited base 15m: failure #1 → 15m.
        assert_eq!(backoff_window_secs("rate_limited", 1), 15 * 60);
        // A large streak saturates at the cap, never overflows.
        assert_eq!(backoff_window_secs("rate_limited", 100), BACKOFF_MAX_SECS);
        // A successful last check (no streak) implies no backoff.
        assert_eq!(backoff_window_secs("ok", 0), 0);
    }

    #[test]
    fn future_timestamp_from_clock_jump_does_not_skip() {
        let now = at("2026-07-05T12:00:00Z");
        let future = gate(Some("2026-07-05T12:30:00Z"), "ok", 0);
        assert!(!should_skip_scan(now, &future, false));
    }

    #[test]
    fn error_messages_classify_rate_limits() {
        assert_eq!(
            health_for_error("Bandcamp rate limit exceeded (429)"),
            FollowHealth::RateLimited
        );
        assert_eq!(
            health_for_error("Discogs API returned status 429 Too Many Requests"),
            FollowHealth::RateLimited
        );
        assert_eq!(
            health_for_error("Failed to fetch page: connection reset"),
            FollowHealth::Error
        );
    }
}
