use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{Emitter, Manager, State};

use crate::error::{CrateError, Result};
use crate::models::{
    BulkImportProgress, BulkImportResult, DiscoveryFilter, DiscoveryRelease,
    DiscoveryReleaseCreate, DiscoveryReleaseUpdate, DiscoveryTrack, DiscoveryTrackCreate,
    PreviewStream, ScannedPage, ScannedRelease,
};
use crate::services::discovery::metadata::{self, FetchedMetadata};
use crate::services::discovery::n_transform::{self, NsigSolverState};
use crate::services::discovery::streams::{self, StreamInfo};
use crate::services::discovery::CachedStream;
use crate::services::DiscoveryService;
// LibraryService + TagService are only used by `purchase_discovery_release` (desktop-only:
// imports purchased files into the library).
#[cfg(feature = "desktop")]
use crate::services::{LibraryService, TagService};
// `ImportResultWithDuplicates` is only the return type of `purchase_discovery_release`.
#[cfg(feature = "desktop")]
use crate::models::ImportResultWithDuplicates;
use crate::{
    AvatarCache, BulkImportCancelFlag, EnrichmentSkipIds, ProxyRestartSignal, ProxyServerPort,
    ScanEnrichmentCache, ScanPageCancelFlag, StreamFetchPermits,
};

// Stream resolution is fully lazy: URLs are fetched when the user plays a track (plus
// the playback queue's small bounded look-ahead, which calls `fetch_preview_stream`
// with `background: true`). There is deliberately no import/enrich-time prefetch —
// resolved stream URLs expire within hours (SoundCloud ~1h, YouTube ~5h, Bandcamp
// ~6h), so warming thousands of releases was wasted network and DB contention.

#[tauri::command]
pub async fn create_discovery_release(
    create: DiscoveryReleaseCreate,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.create_release(create)
}

#[tauri::command]
pub async fn fetch_preview_stream(
    release_id: String,
    track_position: i32,
    background: Option<bool>,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    proxy_port: State<'_, ProxyServerPort>,
    permits: State<'_, StreamFetchPermits>,
) -> Result<PreviewStream> {
    let result = async {
        let port = proxy_port.0;
        let app_data_dir = discovery.app_data_dir();

        if let Some(direct) = cached_file_stream(&discovery, &release_id, track_position) {
            log::info!("Preview stream {release_id}/{track_position}: serving cached file directly");
            return Ok(direct);
        }

        // Everything from here on is served through the localhost proxy — make sure its
        // listener actually accepts connections before handing a URL to a media player.
        let restart = app.state::<ProxyRestartSignal>().0.clone();
        crate::proxy::ensure_proxy_alive(port, &restart).await;

        // Check if audio bytes are already cached on disk — skip stream URL resolution entirely
        if discovery
            .get_cached_audio_meta(&release_id, track_position)
            .unwrap_or(None)
            .is_some()
        {
            log::info!("Preview stream {release_id}/{track_position}: serving from audio cache");
            return Ok(PreviewStream {
                url: format!("http://127.0.0.1:{port}/{release_id}/{track_position}"),
                mime_type: None,
            });
        }

        // Check stream URL cache
        if let Some(cached) = discovery.get_cached_stream(&release_id, track_position)? {
            log::info!("Preview stream {release_id}/{track_position}: using cached stream URL");
            return Ok(resolve_stream_url(
                &cached,
                &release_id,
                track_position,
                port,
            ));
        }

        // Get release to determine source type and URL
        let release = discovery.get_release(&release_id)?;
        log::info!(
            "Preview stream {release_id}/{track_position}: resolving fresh ({})",
            release.source_type
        );

        // Everything below performs a network extraction. Foreground fetches (the track the
        // user just tapped) never wait; opportunistic background resolution (queue window
        // tails, offline pre-caching) throttles through the global permit pool so it can't
        // starve a tap-to-play or hammer the source platforms.
        let _permit = if background.unwrap_or(false) {
            Some(
                permits
                    .0
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| CrateError::Discovery("stream fetch permits closed".into()))?,
            )
        } else {
            None
        };

        // YouTube fast path: use stored video_id for single-track fetch (~500ms vs ~8s)
        if release.source_type == "youtube" {
            if let Some(video_id) = discovery.get_video_id_for_track(&release_id, track_position)? {
                let mut stream =
                    streams::extract_single_youtube_stream(&video_id, track_position).await?;
                transform_youtube_n_params(std::slice::from_mut(&mut stream), &app, &app_data_dir)
                    .await;
                discovery.cache_streams(&release_id, std::slice::from_ref(&stream))?;

                let cached = CachedStream {
                    stream_url: stream.stream_url.clone(),
                    proxy_ua: stream.proxy_ua.clone(),
                };
                return Ok(resolve_stream_url(
                    &cached,
                    &release_id,
                    track_position,
                    port,
                ));
            }
            // Fall through to full extraction for pre-migration releases without video_id
        }

        // Discogs: stream via stored YouTube video_id
        if release.source_type == "discogs" {
            return match discovery.get_video_id_for_track(&release_id, track_position)? {
                Some(video_id) => {
                    let mut stream =
                        streams::extract_single_youtube_stream(&video_id, track_position).await?;
                    transform_youtube_n_params(
                        std::slice::from_mut(&mut stream),
                        &app,
                        &app_data_dir,
                    )
                    .await;
                    discovery.cache_streams(&release_id, std::slice::from_ref(&stream))?;
                    let cached = CachedStream {
                        stream_url: stream.stream_url.clone(),
                        proxy_ua: stream.proxy_ua.clone(),
                    };
                    Ok(resolve_stream_url(
                        &cached,
                        &release_id,
                        track_position,
                        port,
                    ))
                }
                None => Err(CrateError::Discovery(
                    "No YouTube video available for this Discogs track".into(),
                )),
            };
        }

        let mut stream_infos = match release.source_type.as_str() {
            "bandcamp" => streams::extract_bandcamp_streams(&release.url).await?,
            "soundcloud" => {
                let cached_cid = discovery.get_cached_sc_client_id()?;
                let (infos, new_cid) =
                    streams::extract_soundcloud_streams(&release.url, cached_cid).await?;
                discovery.cache_sc_client_id(&new_cid)?;
                infos
            }
            "youtube" => streams::extract_youtube_streams(&release.url).await?,
            other => {
                return Err(CrateError::Discovery(format!(
                    "Preview not supported for source type: {other}"
                )));
            }
        };

        transform_youtube_n_params(&mut stream_infos, &app, &app_data_dir).await;

        // Cache all extracted streams
        discovery.cache_streams(&release_id, &stream_infos)?;

        // A successful extraction is ground truth for the whole release: any track
        // position it did NOT return has no preview at the source right now (e.g. the
        // unreleased tracks of a Bandcamp pre-order). Record that so the UI can grey
        // those tracks out and the queue stops re-fetching the page for them.
        let unavailable: Vec<i32> = release
            .tracks
            .iter()
            .map(|t| t.position)
            .filter(|p| !stream_infos.iter().any(|s| s.track_position == *p))
            .collect();
        discovery.set_preview_availability(&release_id, &unavailable)?;
        // Let the UI grey out / un-grey rows immediately — flags changed in the DB, but the
        // frontend's in-memory releases won't otherwise learn until the next full reload.
        let _ = app.emit(
            "discovery-availability-changed",
            serde_json::json!({ "releaseId": release_id, "unavailable": unavailable }),
        );

        // Return the requested track's URL (direct or proxied)
        let stream = stream_infos
            .iter()
            .find(|s| s.track_position == track_position)
            .ok_or_else(|| {
                CrateError::Discovery(format!(
                    "No stream found for track position {track_position}"
                ))
            })?;

        let cached = CachedStream {
            stream_url: stream.stream_url.clone(),
            proxy_ua: stream.proxy_ua.clone(),
        };
        Ok(resolve_stream_url(
            &cached,
            &release_id,
            track_position,
            port,
        ))
    }
    .await;

    // Resolution failures were previously invisible in device logs — the error only surfaced
    // as a generic toast in the webview, making field failures undiagnosable.
    if let Err(e) = &result {
        log::warn!(
            "Preview stream {release_id}/{track_position} resolution failed (background={}): {e}",
            background.unwrap_or(false)
        );
    }
    result
}

/// Always route through the localhost proxy for unified disk caching.
/// The proxy uses `proxy_ua` from the stream cache when set (YouTube/Discogs),
/// or a default user-agent when not (Bandcamp/SoundCloud). The proxy response
/// carries its own Content-Type header, so no out-of-band MIME is needed here.
fn resolve_stream_url(
    _cached: &CachedStream,
    release_id: &str,
    track_position: i32,
    proxy_port: u16,
) -> PreviewStream {
    PreviewStream {
        url: format!("http://127.0.0.1:{proxy_port}/{release_id}/{track_position}"),
        mime_type: None,
    }
}

/// iOS: play fully-cached audio straight off disk instead of through the localhost proxy.
/// `AVPlayer` reads file URLs natively, so a downloaded track keeps playing with no HTTP
/// involved at all — which is what makes cached playback survive iOS closing the app's
/// listening sockets on suspend (the failure mode where every replay died with
/// `NSURLErrorDomain -1004`). The cache files are extensionless, so the recorded MIME type
/// rides along for `AVURLAssetOverrideMIMETypeKey`.
///
/// The on-disk size must match the recorded size, mirroring the proxy's own guard: a
/// mismatch falls through to the proxy, which owns stale-entry recovery.
#[cfg(target_os = "ios")]
fn cached_file_stream(
    discovery: &DiscoveryService,
    release_id: &str,
    track_position: i32,
) -> Option<PreviewStream> {
    let (content_type, file_size) = discovery
        .get_cached_audio_meta(release_id, track_position)
        .unwrap_or(None)?;
    let path = discovery.audio_cache_path(release_id, track_position);
    if std::fs::metadata(&path).map(|m| m.len() as i64).ok() != Some(file_size) {
        return None;
    }
    // Keep the LRU honest: a direct-played track never reaches the proxy's bump.
    let _ = discovery.touch_audio_cache_access(release_id, track_position);
    Some(PreviewStream {
        url: file_url(&path),
        mime_type: Some(content_type),
    })
}

/// Desktop and Android play through the proxy (HTML5 `<audio>` can't load a `file://` URL
/// from the webview origin), so there is no direct-file fast path there.
#[cfg(not(target_os = "ios"))]
fn cached_file_stream(
    _discovery: &DiscoveryService,
    _release_id: &str,
    _track_position: i32,
) -> Option<PreviewStream> {
    None
}

/// Build a `file://` URL from an absolute path, percent-encoding every byte RFC 3986
/// keeps out of a path segment — the iOS app-data dir contains "Application Support",
/// and `NSURL URLWithString:` returns nil on a raw space.
#[cfg(target_os = "ios")]
fn file_url(path: &std::path::Path) -> String {
    use std::fmt::Write;

    let mut out = String::from("file://");
    for &b in path.to_string_lossy().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// Re-extract a release's streams purely to refresh per-track preview availability
/// (and opportunistically warm the stream-URL cache). The frontend fires this in the
/// background when it shows a release with unavailable tracks whose release date has
/// passed — the path by which a pre-order un-greys itself once the album is out.
/// Bandcamp/SoundCloud only; other source types return the current tracks unchanged.
#[tauri::command]
pub async fn recheck_preview_availability(
    release_id: String,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    permits: State<'_, StreamFetchPermits>,
) -> Result<Vec<DiscoveryTrack>> {
    let release = discovery.get_release(&release_id)?;
    if release.source_type != "bandcamp" && release.source_type != "soundcloud" {
        return Ok(release.tracks);
    }

    // Opportunistic background work — throttle through the shared permit pool so a
    // burst of rechecks can't starve tap-to-play or hammer the source platforms.
    let _permit = permits
        .0
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| CrateError::Discovery("stream fetch permits closed".into()))?;

    let stream_infos = match release.source_type.as_str() {
        "bandcamp" => streams::extract_bandcamp_streams(&release.url).await?,
        _ => {
            let cached_cid = discovery.get_cached_sc_client_id()?;
            let (infos, new_cid) =
                streams::extract_soundcloud_streams(&release.url, cached_cid).await?;
            discovery.cache_sc_client_id(&new_cid)?;
            infos
        }
    };

    discovery.cache_streams(&release_id, &stream_infos)?;

    let unavailable: Vec<i32> = release
        .tracks
        .iter()
        .map(|t| t.position)
        .filter(|p| !stream_infos.iter().any(|s| s.track_position == *p))
        .collect();
    log::info!(
        "Availability recheck {release_id}: {} of {} tracks unavailable",
        unavailable.len(),
        release.tracks.len()
    );
    discovery.set_preview_availability(&release_id, &unavailable)?;
    // The caller applies the returned tracks itself; the event covers every OTHER in-memory
    // holder (the playlist store's copies, a second window) the same way the play path does.
    let _ = app.emit(
        "discovery-availability-changed",
        serde_json::json!({ "releaseId": release_id, "unavailable": unavailable }),
    );

    Ok(discovery.get_release(&release_id)?.tracks)
}

#[tauri::command]
pub async fn get_discovery_audio_cache_size(discovery: State<'_, DiscoveryService>) -> Result<i64> {
    discovery.get_audio_cache_total_size()
}

#[tauri::command]
pub async fn clear_discovery_audio_cache(
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.clear_audio_cache()?;
    let _ = app.emit("discovery-cache-changed", ());
    Ok(())
}

/// Download + cache a release's remote cover to disk for offline (airplane-mode) rendering.
/// Idempotent: an already-cached cover is just touched and re-served. Returns the relative
/// cache path ("discovery/artwork/{id}.ext"), or `None` when the release has no `artwork_url`
/// or the fetch fails (the frontend then falls back to the remote URL).
#[tauri::command]
pub async fn cache_release_artwork(
    release_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<Option<String>> {
    discovery.cache_release_artwork(&release_id).await
}

#[tauri::command]
pub async fn get_discovery_artwork_cache_size(
    discovery: State<'_, DiscoveryService>,
) -> Result<i64> {
    discovery.get_artwork_cache_total_size()
}

#[tauri::command]
pub async fn clear_discovery_artwork_cache(discovery: State<'_, DiscoveryService>) -> Result<()> {
    discovery.clear_artwork_cache()
}

#[tauri::command]
pub async fn invalidate_preview_stream_cache(
    release_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.invalidate_stream_cache(&release_id)
}

/// Delete a release's downloaded audio bytes and cached stream URLs ("Remove Download").
#[tauri::command]
pub async fn purge_release_audio_cache(
    release_id: String,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.purge_release_audio(&release_id)?;
    let _ = app.emit("discovery-cache-changed", ());
    Ok(())
}

/// Per-release cache state for the "downloaded for offline" indicator.
#[derive(serde::Serialize)]
pub struct ReleaseCacheState {
    pub cached_tracks: i64,
    pub total_tracks: i64,
    pub bytes: i64,
    /// Any track pinned via "Download for Offline" (excluded from LRU eviction).
    pub pinned: bool,
}

/// Report how many of a release's tracks have their audio cached on disk (drives the
/// per-release download indicator).
#[tauri::command]
pub async fn get_release_cache_state(
    release_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<ReleaseCacheState> {
    let (cached_tracks, total_tracks, bytes, pinned) =
        discovery.get_release_cache_state(&release_id)?;
    Ok(ReleaseCacheState {
        cached_tracks,
        total_tracks,
        bytes,
        pinned,
    })
}

/// Bulk cached-state for list badges / the Downloaded filter: one entry per release that has
/// at least one track's audio on disk. Consumers refetch on `discovery-cache-changed`.
#[derive(serde::Serialize)]
pub struct CachedReleaseState {
    pub release_id: String,
    pub cached_tracks: i64,
    pub total_tracks: i64,
    pub fully_cached: bool,
    pub pinned: bool,
}

#[tauri::command]
pub async fn get_cached_release_states(
    discovery: State<'_, DiscoveryService>,
) -> Result<Vec<CachedReleaseState>> {
    Ok(discovery
        .get_cached_release_states()?
        .into_iter()
        .map(
            |(release_id, cached_tracks, total_tracks, pinned)| CachedReleaseState {
                release_id,
                cached_tracks,
                total_tracks,
                fully_cached: total_tracks > 0 && cached_tracks >= total_tracks,
                pinned,
            },
        )
        .collect())
}

/// Proactively download and cache a track's audio bytes for offline playback ("Download for
/// offline" / re-cache on demand). Resolves the stream URL via [`fetch_preview_stream`], then
/// makes a server-side request to the localhost proxy — which downloads the full stream and
/// persists it to the on-disk audio cache (with LRU eviction) — so the track later plays with
/// no network. Idempotent: a request for an already-cached track just re-serves from disk. The
/// frontend calls this per track to drive a whole-release download. Server-side on purpose: the
/// WebView CSP `connect-src` doesn't allow `fetch()` to the proxy, only `<audio>` media loads.
#[tauri::command]
pub async fn precache_preview_stream(
    release_id: String,
    track_position: i32,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    proxy_port: State<'_, ProxyServerPort>,
    permits: State<'_, StreamFetchPermits>,
) -> Result<()> {
    // The handle outlives the state borrows moved into `fetch_preview_stream` below; the
    // pin + event need the service again afterwards.
    let app_handle = app.clone();

    // Resolve (and cache) the stream URL, and get the localhost proxy URL for the track.
    // Background priority: a whole-release offline download must never starve a
    // tap-to-play (cache hits skip the permit entirely).
    let stream = fetch_preview_stream(
        release_id.clone(),
        track_position,
        Some(true),
        app,
        discovery,
        proxy_port,
        permits,
    )
    .await?;

    // A `file://` URL means the audio is already fully on disk (the iOS direct-play fast
    // path) — nothing to download; fall through so the pin + event below still land.
    if !stream.url.starts_with("file://") {
        // Hitting the proxy forces it to download the full stream and persist it to the
        // on-disk cache before responding. A tiny range keeps the transferred body to a
        // couple of bytes while the server still caches everything.
        let client = reqwest::Client::new();
        client
            .get(&stream.url)
            .header("Range", "bytes=0-1")
            .send()
            .await
            .map_err(|e| CrateError::Discovery(format!("Precache proxy request failed: {e:#}")))?
            .error_for_status()
            .map_err(|e| {
                CrateError::Discovery(format!("Precache proxy returned error: {e:#}"))
            })?;
    }

    // Explicit download ⇒ pin, excluding the track from LRU eviction until "Remove Download"
    // (or "Clear cache"). The proxy recorded the cache row before responding, so the pin
    // always lands; the event covers the already-cached → pin-only case where the proxy
    // serves from disk without a fresh download.
    let discovery = app_handle.state::<DiscoveryService>();
    if let Err(e) = discovery.set_audio_cache_pinned(&release_id, track_position, true) {
        log::warn!("Failed to pin downloaded track {release_id}/{track_position}: {e}");
    }
    let _ = app_handle.emit("discovery-cache-changed", ());

    Ok(())
}

#[tauri::command]
pub async fn toggle_discovery_track_liked(
    track_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<bool> {
    discovery.toggle_track_liked(&track_id)
}

#[tauri::command]
pub async fn get_discovery_release(
    id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.get_release(&id)
}

#[tauri::command]
pub async fn get_discovery_releases(
    filter: Option<DiscoveryFilter>,
    discovery: State<'_, DiscoveryService>,
) -> Result<Vec<DiscoveryRelease>> {
    discovery.get_releases(filter)
}

#[tauri::command]
pub async fn update_discovery_release(
    id: String,
    update: DiscoveryReleaseUpdate,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.update_release(&id, update)
}

#[tauri::command]
pub async fn delete_discovery_release(
    id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.delete_release(&id)
}

#[tauri::command]
pub async fn delete_discovery_releases(
    ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.delete_releases(ids)
}

#[tauri::command]
pub async fn assign_discovery_tags(
    release_ids: Vec<String>,
    tag_ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.assign_tags(release_ids, tag_ids)
}

#[tauri::command]
pub async fn remove_discovery_tags(
    release_ids: Vec<String>,
    tag_ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.remove_tags(release_ids, tag_ids)
}

#[tauri::command]
pub async fn check_discovery_matches(
    url: Option<String>,
    artist: Option<String>,
    title: Option<String>,
    parent_url: Option<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<Vec<DiscoveryRelease>> {
    discovery.find_matching_releases(
        url.as_deref(),
        artist.as_deref(),
        title.as_deref(),
        parent_url.as_deref(),
    )
}

#[tauri::command]
pub async fn add_tracks_to_discovery_release(
    release_id: String,
    tracks: Vec<DiscoveryTrackCreate>,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.add_tracks_to_release(&release_id, tracks)
}

#[tauri::command]
pub async fn merge_discovery_releases(
    target_id: String,
    source_ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.merge_releases(&target_id, source_ids)
}

#[tauri::command]
pub async fn fetch_release_metadata(url: String) -> Result<FetchedMetadata> {
    metadata::fetch_metadata(&url).await
}

/// Fetch (and session-cache) the profile/avatar image URL for an artist/label page, so the
/// follow popover can preview who you're about to follow without a follow + scan.
#[tauri::command]
pub async fn fetch_source_avatar(
    url: String,
    cache: State<'_, AvatarCache>,
) -> Result<Option<String>> {
    {
        let map = cache.0.lock().await;
        if let Some(cached) = map.get(&url) {
            return Ok(cached.clone());
        }
    }
    match metadata::fetch_page_avatar(&url).await {
        Ok(avatar) => {
            cache.0.lock().await.insert(url, avatar.clone());
            Ok(avatar)
        }
        Err(e) => {
            log::warn!("fetch_source_avatar failed for {url}: {e}");
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn refresh_release_metadata(
    id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    let release = discovery.get_release(&id)?;
    let fetched = metadata::fetch_metadata(&release.url).await?;

    // If the release has no tracks but fetched data does, create them (fixes bulk-imported releases)
    if release.tracks.is_empty() && !fetched.tracks.is_empty() {
        let track_creates = fetched
            .tracks
            .iter()
            .map(|t| DiscoveryTrackCreate {
                name: t.name.clone(),
                position: t.position,
                duration_ms: t.duration_ms,
                video_id: t.video_id.clone(),
                url: t.url.clone(),
            })
            .collect();
        discovery.add_tracks_to_release(&id, track_creates)?;
    } else {
        // Backfill any missing track durations, video_ids, and track urls from fetched data
        discovery.update_track_durations(&id, &fetched.tracks)?;
        discovery.update_track_video_ids(&id, &fetched.tracks)?;
        discovery.update_track_urls(&id, &fetched.tracks)?;
    }

    let mut update = DiscoveryReleaseUpdate::default();
    if let Some(artist) = fetched.artist {
        update.artist = Some(artist);
    }
    if let Some(title) = fetched.title {
        update.title = Some(title);
    }
    if let Some(label) = fetched.label {
        update.label = Some(label);
    }
    if let Some(release_date) = fetched.release_date {
        update.release_date = Some(release_date);
    }
    if let Some(artwork_url) = fetched.artwork_url {
        update.artwork_url = Some(artwork_url);
    }

    discovery.update_release(&id, update)
}

#[tauri::command]
pub async fn set_discovery_release_artwork(
    release_id: String,
    file_path: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.set_release_artwork(&release_id, &PathBuf::from(file_path))
}

#[tauri::command]
pub async fn delete_discovery_release_artwork(
    release_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.delete_release_artwork(&release_id)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn purchase_discovery_release(
    release_id: String,
    file_paths: Vec<String>,
    transfer_tags: bool,
    remove_after_import: bool,
    discovery: State<'_, DiscoveryService>,
    library: State<'_, LibraryService>,
    tag: State<'_, TagService>,
) -> Result<ImportResultWithDuplicates> {
    let release = discovery.get_release(&release_id)?;

    let pathbufs: Vec<PathBuf> = file_paths.into_iter().map(PathBuf::from).collect();
    let result = library.import_tracks_with_duplicate_detection(pathbufs)?;

    if transfer_tags && !release.tags.is_empty() && !result.tracks.is_empty() {
        let track_ids: Vec<String> = result.tracks.iter().map(|t| t.id.clone()).collect();
        let tag_ids: Vec<String> = release.tags.iter().map(|t| t.id.clone()).collect();
        tag.assign_tags(track_ids, tag_ids)?;
    }

    if remove_after_import {
        discovery.delete_release(&release_id)?;
    }

    Ok(result)
}

/// Callback command invoked from the WebView after the EJS solver completes.
/// Receives the solver result (or error) and unblocks the waiting Rust future.
#[tauri::command]
pub async fn nsig_solve_callback(
    request_id: String,
    result: Option<String>,
    error: Option<String>,
    state: State<'_, NsigSolverState>,
) -> Result<()> {
    let response = error
        .map(|e| format!(r#"{{"type":"error","error":"{e}"}}"#))
        .unwrap_or_else(|| result.unwrap_or_default());
    if let Some(tx) = state.pending.lock().await.remove(&request_id) {
        let _ = tx.send(response);
    }
    Ok(())
}

#[tauri::command]
pub async fn scan_discovery_page(
    url: String,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    cancel_flag: State<'_, ScanPageCancelFlag>,
    enrichment_cache: State<'_, ScanEnrichmentCache>,
) -> Result<ScannedPage> {
    // Cancel any running enrichment from a previous scan
    cancel_flag.0.store(true, Ordering::SeqCst);
    enrichment_cache.0.lock().await.clear();
    cancel_flag.0.store(false, Ordering::SeqCst);

    log::info!("scan_discovery_page: starting scan for {url}");

    let existing_urls = discovery.get_all_release_urls()?;

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        metadata::scan_page(&url, &existing_urls, &cancel_flag.0, Some(&app)),
    )
    .await
    .map_err(|_| CrateError::Discovery("Scan timed out after 60 seconds".into()))??;

    log::info!(
        "scan_discovery_page: returning {} releases",
        result.releases.len()
    );

    // Spawn background enrichment for Discogs releases (pre-fetch tracks while user browses)
    if result.source_type == "discogs" {
        let urls_to_enrich: Vec<String> = result
            .releases
            .iter()
            .filter(|r| !r.already_exists)
            .map(|r| r.url.clone())
            .collect();

        if !urls_to_enrich.is_empty() {
            let cache = enrichment_cache.0.clone();
            let cancel = cancel_flag.0.clone();
            tokio::spawn(async move {
                log::info!(
                    "Starting background enrichment for {} Discogs releases",
                    urls_to_enrich.len()
                );
                for url in &urls_to_enrich {
                    if cancel.load(Ordering::SeqCst) {
                        log::info!("Background enrichment cancelled");
                        break;
                    }
                    // Always delay to respect Discogs rate limits after the scan
                    tokio::time::sleep(metadata::jittered_delay(2500)).await;
                    match metadata::fetch_metadata(url).await {
                        Ok(fetched) => {
                            log::info!("Enriched metadata for {url}");
                            cache.lock().await.insert(url.clone(), fetched);
                        }
                        Err(e) => log::warn!("Enrichment failed for {url}: {e}"),
                    }
                }
                log::info!("Background enrichment complete");
            });
        }
    }

    Ok(result)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn bulk_create_discovery_releases(
    urls: Vec<String>,
    scanned_releases: Option<Vec<ScannedRelease>>,
    page_label: Option<String>,
    page_artist: Option<String>,
    source_type: Option<String>,
    page_url: Option<String>,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    cancel_flag: State<'_, BulkImportCancelFlag>,
    scan_cancel_flag: State<'_, ScanPageCancelFlag>,
    enrichment_cache: State<'_, ScanEnrichmentCache>,
    enrichment_skip_ids: State<'_, EnrichmentSkipIds>,
) -> Result<BulkImportResult> {
    // Reset cancel flag and enrichment skip set
    cancel_flag.0.store(false, Ordering::SeqCst);
    enrichment_skip_ids.0.lock().await.clear();

    // Cancel background enrichment to avoid competing Discogs API calls
    scan_cancel_flag.0.store(true, Ordering::SeqCst);

    // Build a lookup of scanned data by URL for the fast path
    let scanned_map: std::collections::HashMap<String, ScannedRelease> = scanned_releases
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.url.clone(), r))
        .collect();

    // Collect releases created from scanned data (no tracks) for background enrichment
    let mut needs_enrichment: Vec<(String, String, String)> = Vec::new();

    let total = urls.len();
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut failed_urls = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        // Check for cancellation
        if cancel_flag.0.load(Ordering::SeqCst) {
            log::info!("Bulk import cancelled at {}/{total}", i + 1);
            break;
        }

        // Try fast path: use scanned data if available (no API call, no throttle)
        let (create, current_title) = if let Some(scanned) = scanned_map.get(url) {
            // For Discogs, check if background enrichment pre-fetched full metadata
            let enriched = if source_type.as_deref() == Some("discogs") {
                enrichment_cache.0.lock().await.remove(url)
            } else {
                None
            };

            if let Some(fetched) = enriched {
                // Enriched path: has tracks from background fetch
                let create = DiscoveryReleaseCreate {
                    url: url.clone(),
                    source_type: source_type.clone(),
                    artist: fetched
                        .artist
                        .or(scanned.artist.clone())
                        .or(page_artist.clone()),
                    title: fetched.title.or(scanned.title.clone()),
                    label: fetched.label.or(page_label.clone()),
                    release_date: fetched.release_date.or(scanned.release_date.clone()),
                    artwork_url: fetched.artwork_url.or(scanned.artwork_url.clone()),
                    notes: None,
                    parent_url: fetched.parent_url,
                    source_page_url: page_url.clone(),
                    tracks: if fetched.tracks.is_empty() {
                        None
                    } else {
                        Some(
                            fetched
                                .tracks
                                .iter()
                                .map(|t| DiscoveryTrackCreate {
                                    name: t.name.clone(),
                                    position: t.position,
                                    duration_ms: t.duration_ms,
                                    video_id: t.video_id.clone(),
                                    url: t.url.clone(),
                                })
                                .collect(),
                        )
                    },
                };
                (Ok(create), scanned.title.clone())
            } else {
                // Fast path: use scanned data only (no tracks)
                let create = DiscoveryReleaseCreate {
                    url: url.clone(),
                    source_type: source_type.clone(),
                    artist: scanned.artist.clone().or(page_artist.clone()),
                    title: scanned.title.clone(),
                    label: page_label.clone(),
                    release_date: scanned.release_date.clone(),
                    artwork_url: scanned.artwork_url.clone(),
                    notes: None,
                    parent_url: None,
                    source_page_url: page_url.clone(),
                    tracks: None,
                };
                (Ok(create), scanned.title.clone())
            }
        } else {
            // Slow path: throttle and fetch metadata individually
            if i > 0 {
                let delay = if url.to_lowercase().contains("discogs.com") {
                    metadata::jittered_delay(2500)
                } else {
                    metadata::jittered_delay(500)
                };
                tokio::time::sleep(delay).await;
            }

            match metadata::fetch_metadata(url).await {
                Ok(fetched) => {
                    let title = fetched.title.clone();
                    let create = DiscoveryReleaseCreate {
                        url: url.clone(),
                        source_type: Some(fetched.source_type.clone()),
                        artist: fetched.artist.clone().or(page_artist.clone()),
                        title: fetched.title.clone(),
                        label: fetched.label.clone().or(page_label.clone()),
                        release_date: fetched.release_date.clone(),
                        artwork_url: fetched.artwork_url.clone(),
                        notes: None,
                        parent_url: fetched.parent_url.clone(),
                        source_page_url: page_url.clone(),
                        tracks: if fetched.tracks.is_empty() {
                            None
                        } else {
                            Some(
                                fetched
                                    .tracks
                                    .iter()
                                    .map(|t| DiscoveryTrackCreate {
                                        name: t.name.clone(),
                                        position: t.position,
                                        duration_ms: t.duration_ms,
                                        video_id: t.video_id.clone(),
                                        url: t.url.clone(),
                                    })
                                    .collect(),
                            )
                        },
                    };
                    (Ok(create), title)
                }
                Err(e) => {
                    log::warn!("Bulk import: failed to fetch metadata for {url}: {e}");
                    (Err(e), None)
                }
            }
        };

        match create {
            Ok(create_data) => {
                let has_tracks = create_data.tracks.as_ref().is_some_and(|t| !t.is_empty());
                // Create the release — catch UNIQUE constraint errors as skipped (not failed)
                match discovery.create_release(create_data) {
                    Ok(release) => {
                        succeeded += 1;
                        if !has_tracks {
                            // Track for background enrichment
                            needs_enrichment.push((
                                release.id.clone(),
                                release.url.clone(),
                                release.source_type.clone(),
                            ));
                        }
                    }
                    Err(CrateError::Database(rusqlite::Error::SqliteFailure(err, _)))
                        if err.code == rusqlite::ffi::ErrorCode::ConstraintViolation =>
                    {
                        // Already exists — treat as skipped, not failed. Backfill the
                        // discovered-from page so re-scanning a label repairs releases
                        // imported before this column existed.
                        if let Some(ref page) = page_url {
                            let _ = discovery.set_source_page_url_if_absent(url, page);
                        }
                        succeeded += 1;
                    }
                    Err(e) => {
                        log::warn!("Bulk import: failed to create release for {url}: {e}");
                        failed += 1;
                        failed_urls.push(url.clone());
                    }
                }
            }
            Err(_) => {
                failed += 1;
                failed_urls.push(url.clone());
            }
        }

        let _ = app.emit(
            "bulk-import-progress",
            BulkImportProgress {
                current: i + 1,
                total,
                current_title,
                succeeded,
                failed,
            },
        );
    }

    // Spawn background enrichment for releases created without tracks
    if !needs_enrichment.is_empty() {
        // Notify frontend so it can show spinners on releases that will be enriched
        let enrichment_ids: Vec<String> = needs_enrichment
            .iter()
            .map(|(id, _, _)| id.clone())
            .collect();
        let _ = app.emit("discovery-enrichment-queued", &enrichment_ids);

        let db = discovery.db();
        let app_data_dir = discovery.app_data_dir();
        let app_handle = app.clone();
        let skip_ids = enrichment_skip_ids.0.clone();
        tokio::spawn(async move {
            log::info!(
                "Starting background enrichment for {} releases",
                needs_enrichment.len()
            );
            let svc = DiscoveryService::with_db(db, app_data_dir.clone());
            for (idx, (release_id, url, src_type)) in needs_enrichment.iter().enumerate() {
                // Skip if the user cancelled this release
                if skip_ids.lock().await.contains(release_id) {
                    log::info!("Skipping enrichment for {release_id} (cancelled by user)");
                    continue;
                }

                // Throttle between requests
                if idx > 0 {
                    let delay = if src_type == "discogs" {
                        metadata::jittered_delay(2500)
                    } else {
                        metadata::jittered_delay(500)
                    };
                    tokio::time::sleep(delay).await;
                }

                // Check again after the delay in case the user cancelled during the wait
                if skip_ids.lock().await.contains(release_id) {
                    log::info!("Skipping enrichment for {release_id} (cancelled by user)");
                    continue;
                }

                // Fetch full metadata
                let fetched = match metadata::fetch_metadata(url).await {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("Background enrichment failed for {release_id}: {e}");
                        continue;
                    }
                };

                // Check if release still has no tracks (may have been manually refreshed)
                let existing = match svc.get_release(release_id) {
                    Ok(r) => r,
                    Err(e) => {
                        log::warn!(
                            "Background enrichment: could not fetch release {release_id}: {e}"
                        );
                        continue;
                    }
                };

                if existing.tracks.is_empty() && !fetched.tracks.is_empty() {
                    let track_creates = fetched
                        .tracks
                        .iter()
                        .map(|t| DiscoveryTrackCreate {
                            name: t.name.clone(),
                            position: t.position,
                            duration_ms: t.duration_ms,
                            video_id: t.video_id.clone(),
                            url: t.url.clone(),
                        })
                        .collect();
                    if let Err(e) = svc.add_tracks_to_release(release_id, track_creates) {
                        log::warn!(
                            "Background enrichment: failed to add tracks to {release_id}: {e}"
                        );
                    }
                } else if !existing.tracks.is_empty() {
                    // Backfill missing durations, video IDs, and track urls
                    let _ = svc.update_track_durations(release_id, &fetched.tracks);
                    let _ = svc.update_track_video_ids(release_id, &fetched.tracks);
                    let _ = svc.update_track_urls(release_id, &fetched.tracks);
                }

                // Update release metadata fields
                let mut update = DiscoveryReleaseUpdate::default();
                if let Some(artist) = fetched.artist {
                    update.artist = Some(artist);
                }
                if let Some(title) = fetched.title {
                    update.title = Some(title);
                }
                if let Some(label) = fetched.label {
                    update.label = Some(label);
                }
                if let Some(release_date) = fetched.release_date {
                    update.release_date = Some(release_date);
                }
                if let Some(artwork_url) = fetched.artwork_url {
                    update.artwork_url = Some(artwork_url);
                }

                match svc.update_release(release_id, update) {
                    Ok(updated_release) => {
                        // Emit event so the frontend updates in real-time. Stream URLs
                        // resolve lazily on first play — no prefetch here.
                        let _ = app_handle.emit("discovery-release-updated", &updated_release);
                        log::info!("Background enrichment complete for {release_id}");
                    }
                    Err(e) => {
                        log::warn!(
                            "Background enrichment: failed to update release {release_id}: {e}"
                        );
                    }
                }
            }
            log::info!("Background enrichment finished");
        });
    }

    Ok(BulkImportResult {
        succeeded,
        failed,
        failed_urls,
    })
}

#[tauri::command]
pub async fn cancel_bulk_import(cancel_flag: State<'_, BulkImportCancelFlag>) -> Result<()> {
    cancel_flag.0.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn cancel_scan_page(cancel_flag: State<'_, ScanPageCancelFlag>) -> Result<()> {
    cancel_flag.0.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn skip_enrichment(id: String, skip_ids: State<'_, EnrichmentSkipIds>) -> Result<()> {
    skip_ids.0.lock().await.insert(id);
    Ok(())
}

/// Transform the `n` query parameter on YouTube CDN stream URLs (IOS client).
/// Only processes streams that have a `proxy_ua` set (i.e. non-browser-compatible).
async fn transform_youtube_n_params(
    streams: &mut [StreamInfo],
    app_handle: &tauri::AppHandle,
    app_data_dir: &std::path::Path,
) {
    for stream in streams.iter_mut() {
        if stream.proxy_ua.is_some() {
            stream.stream_url =
                n_transform::transform_n_param(&stream.stream_url, app_handle, app_data_dir).await;
        }
    }
}
