use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{CrateError, Result};
use crate::services::DiscoveryService;

use tauri::{Emitter, Manager};

/// Size of each sequential download chunk (~4 MB).
/// With n-param transformation, YouTube CDN allows full downloads for ANDROID_VR client URLs.
const CHUNK_SIZE: u64 = 4_194_304;

/// Maximum total download size to prevent runaway downloads (~50 MB).
const MAX_TOTAL_SIZE: u64 = 52_428_800;

/// Shared state threaded into every axum proxy request handler.
///
/// Serving is DISK-BACKED: a stream downloads once to the on-disk audio cache and every
/// response reads the requested byte slice from the file. Audio bytes are never held in
/// memory beyond the in-flight download chunk — the previous in-memory cache (3 × ≤50 MB
/// entries, plus a full-body clone per 200 response) was a prime WKWebView memory-pressure
/// suspect on mobile.
#[derive(Clone)]
pub(crate) struct ProxyServerState {
    pub app_handle: tauri::AppHandle,
    pub client: reqwest::Client,
    /// In-flight downloads: maps cache key to a watch receiver that signals completion.
    /// `None` = download in progress, `Some(true)` = success, `Some(false)` = failed.
    downloads: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::watch::Receiver<Option<bool>>>>>,
}

impl ProxyServerState {
    pub fn new(app_handle: tauri::AppHandle, client: reqwest::Client) -> Self {
        Self {
            app_handle,
            client,
            downloads: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
}

/// Parse a `bytes=start-[end]` Range header value and return `(start, optional_end)`.
fn parse_bytes_range(header: Option<&str>) -> (u64, Option<u64>) {
    let s = match header {
        Some(h) => h.trim_start_matches("bytes="),
        None => return (0, None),
    };
    let mut parts = s.splitn(2, '-');
    let start = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let end = parts.next().and_then(|v| v.parse().ok());
    (start, end)
}

/// Parse total file size from a `Content-Range: bytes start-end/total` header.
fn parse_total_from_content_range(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let cr = headers.get("Content-Range")?.to_str().ok()?;
    cr.split('/').next_back()?.parse().ok()
}

/// Top-level axum handler for `GET /{release_id}/{track_position}`.
///
/// YouTube stream URLs are signed for non-browser user-agents. The HTML5 Audio element sends a
/// browser UA, causing a 403. This server fetches the stream with the correct UA server-side and
/// proxies the bytes back to the media element. Using a real HTTP server (rather than a custom URI
/// scheme) ensures WKWebView's AVFoundation media layer can seek correctly via Range requests.
pub(crate) async fn proxy_http_handler(
    axum::extract::Path((release_id, track_position)): axum::extract::Path<(String, i32)>,
    req_headers: axum::http::HeaderMap,
    axum::extract::State(state): axum::extract::State<ProxyServerState>,
) -> axum::http::Response<axum::body::Body> {
    match proxy_http_handler_inner(&release_id, track_position, &req_headers, &state).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Stream proxy error: {e}");
            axum::http::Response::builder()
                .status(502)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from("Internal proxy error"))
                .unwrap()
        }
    }
}

async fn proxy_http_handler_inner(
    release_id: &str,
    track_position: i32,
    req_headers: &axum::http::HeaderMap,
    state: &ProxyServerState,
) -> Result<axum::http::Response<axum::body::Body>> {
    let cache_key = format!("{release_id}/{track_position}");
    let incoming_range = req_headers.get("Range").and_then(|v| v.to_str().ok());
    let has_range_header = incoming_range.is_some();
    let (start, end_opt) = parse_bytes_range(incoming_range);

    // 1. Disk cache: serve the requested slice straight from the file.
    {
        let discovery = state.app_handle.state::<DiscoveryService>();
        if let Ok(Some((content_type, file_size))) =
            discovery.get_cached_audio_meta(release_id, track_position)
        {
            let file_path = discovery.audio_cache_path(release_id, track_position);
            let disk_len = std::fs::metadata(&file_path).map(|m| m.len() as i64).ok();
            if disk_len == Some(file_size) {
                // Bump the LRU access time so a replayed track survives eviction.
                let _ = discovery.touch_audio_cache_access(release_id, track_position);
                return serve_range_from_file(
                    &file_path,
                    &content_type,
                    file_size as u64,
                    start,
                    end_opt,
                    has_range_header,
                )
                .await;
            }
            // Only this track's entry is stale — leave the release's other cached (possibly
            // pinned/downloaded) tracks alone.
            log::warn!("Audio cache file size mismatch for {cache_key}, removing stale entry");
            let _ = discovery.delete_cached_audio_track(release_id, track_position);
        }
    }

    // 2. Check if a download is already in progress for this key.
    //    If so, wait for it. If not, start one.
    let mut rx = {
        let mut downloads = state.downloads.lock().await;
        if let Some(existing_rx) = downloads.get(&cache_key) {
            existing_rx.clone()
        } else {
            let discovery = state.app_handle.state::<DiscoveryService>();
            let cached_stream = discovery
                .get_cached_stream(release_id, track_position)?
                .ok_or_else(|| {
                    CrateError::Discovery("No cached stream for proxy request".into())
                })?;

            // Use proxy_ua if set (YouTube/Discogs), empty string otherwise (Bandcamp/SoundCloud)
            let proxy_ua = cached_stream.proxy_ua.unwrap_or_default();

            let (tx, rx) = tokio::sync::watch::channel(None);
            downloads.insert(cache_key.clone(), rx.clone());

            let state_clone = state.clone();
            let url = cached_stream.stream_url.clone();
            let key = cache_key.clone();
            let rid = release_id.to_string();
            let tp = track_position;

            tokio::spawn(async move {
                let discovery = state_clone.app_handle.state::<DiscoveryService>();
                let file_path = discovery.audio_cache_path(&rid, tp);
                let success = match download_stream_to_disk(
                    &state_clone,
                    &url,
                    &proxy_ua,
                    &file_path,
                )
                .await
                {
                    Ok((content_type, size)) => {
                        if let Err(e) =
                            discovery.save_audio_cache_entry(&rid, tp, &content_type, size as i64)
                        {
                            log::warn!("Failed to record audio cache entry: {e}");
                        } else {
                            log::info!("Cached audio to disk: {rid}/{tp} ({size} bytes)");
                            // Keep the on-disk cache under its size cap (LRU eviction).
                            if let Err(e) = discovery.enforce_audio_cache_limit() {
                                log::warn!("Failed to enforce audio cache limit: {e}");
                            }
                            // Covers organic play-writes, precache downloads, AND evictions —
                            // cached-state consumers (badges / downloaded filter) refetch on this.
                            let _ = state_clone.app_handle.emit("discovery-cache-changed", ());
                        }
                        true
                    }
                    Err(e) => {
                        log::warn!("Stream download failed for {key}: {e}");
                        // A cached stream URL can die before its recorded expiry (CDN signatures
                        // are often bound to the resolving IP, so a WiFi↔cellular hop invalidates
                        // them). Without this, every replay re-serves the same dead URL until the
                        // expiry timestamp passes — even across app restarts. URL-level only:
                        // never touches downloaded audio bytes.
                        if let Err(e) = discovery.invalidate_stream_cache(&rid) {
                            log::warn!("Failed to invalidate stream cache for {rid}: {e}");
                        }
                        false
                    }
                };

                let _ = tx.send(Some(success));
                state_clone.downloads.lock().await.remove(&key);
            });

            rx
        }
    };

    // 3. Wait for the download to complete.
    if rx.changed().await.is_err() {
        return Err(CrateError::Discovery(
            "Download task dropped unexpectedly".into(),
        ));
    }

    if *rx.borrow() != Some(true) {
        return Ok(axum::http::Response::builder()
            .status(502)
            .header("Content-Type", "text/plain")
            .body(axum::body::Body::from("Stream download failed"))
            .unwrap());
    }

    // 4. Serve from the freshly written disk cache.
    let discovery = state.app_handle.state::<DiscoveryService>();
    let (content_type, file_size) = discovery
        .get_cached_audio_meta(release_id, track_position)?
        .ok_or_else(|| {
            CrateError::Discovery("Download succeeded but cache entry missing".into())
        })?;
    let file_path = discovery.audio_cache_path(release_id, track_position);
    serve_range_from_file(
        &file_path,
        &content_type,
        file_size as u64,
        start,
        end_opt,
        has_range_header,
    )
    .await
}

/// Download an audio stream in sequential chunks, appending each chunk to
/// `{dest}.part` and renaming to `dest` on completion. Peak memory is one ~4 MB chunk,
/// never the whole file. Returns `(content_type, size_on_disk)`.
///
/// Without n-param transformation, YouTube CDN throttles downloads to ~1 MB per video.
/// With transformation, full downloads are possible. Browser-compatible clients
/// (WEB, WEB_EMBEDDED) don't go through the proxy at all.
async fn download_stream_to_disk(
    state: &ProxyServerState,
    stream_url: &str,
    proxy_ua: &str,
    dest: &Path,
) -> Result<(String, u64)> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part_path: PathBuf = {
        let mut os = dest.as_os_str().to_owned();
        os.push(".part");
        PathBuf::from(os)
    };
    let mut file = std::fs::File::create(&part_path)?;

    let mut written: u64 = 0;
    let mut content_type = String::from("audio/mp4");
    let mut total_size: Option<u64> = None;

    let result: Result<()> = async {
        loop {
            let offset = written;

            if let Some(total) = total_size {
                if offset >= total {
                    break;
                }
            }
            if offset >= MAX_TOTAL_SIZE {
                break;
            }

            let range_end = offset + CHUNK_SIZE - 1;
            let mut req = state
                .client
                .get(stream_url)
                .header("Range", format!("bytes={offset}-{range_end}"));
            if !proxy_ua.is_empty() {
                req = req.header("User-Agent", proxy_ua);
            }
            let response = req.send().await.map_err(|e| {
                CrateError::Discovery(format!("Stream chunk request failed: {e:#}"))
            })?;

            let status = response.status();
            if !status.is_success() && status.as_u16() != 206 {
                if written == 0 {
                    return Err(CrateError::Discovery(format!(
                        "Stream download returned {status}"
                    )));
                }
                // CDN rejected subsequent chunk — cache what we have.
                log::info!(
                    "CDN rejected chunk at offset {offset} ({status}), caching {written} bytes of partial audio"
                );
                break;
            }

            // Extract metadata from the first chunk.
            if offset == 0 {
                total_size = parse_total_from_content_range(response.headers());
                if let Some(ct) = response
                    .headers()
                    .get("Content-Type")
                    .and_then(|v| v.to_str().ok())
                {
                    content_type = ct.to_string();
                }
                log::info!(
                    "Stream download started: total_size={total_size:?}, content-type={content_type}"
                );
            }

            let chunk_bytes = response
                .bytes()
                .await
                .map_err(|e| CrateError::Discovery(format!("Stream chunk body failed: {e:#}")))?;

            if chunk_bytes.is_empty() {
                break;
            }

            file.write_all(&chunk_bytes)?;
            written += chunk_bytes.len() as u64;
        }
        Ok(())
    }
    .await;

    if let Err(e) = result {
        let _ = std::fs::remove_file(&part_path);
        return Err(e);
    }

    if written == 0 {
        let _ = std::fs::remove_file(&part_path);
        return Err(CrateError::Discovery(
            "Stream download produced no data".into(),
        ));
    }

    file.flush()?;
    drop(file);
    std::fs::rename(&part_path, dest)?;

    let full = total_size.is_none_or(|total| written >= total);
    log::info!(
        "Stream download {}: {written} bytes cached, content-type: {content_type}",
        if full { "complete" } else { "partial" },
    );

    Ok((content_type, written))
}

/// Common CORS headers applied to every proxy response.
fn apply_cors_headers(builder: axum::http::response::Builder) -> axum::http::response::Builder {
    builder
        .header("Access-Control-Allow-Origin", "*")
        .header(
            "Access-Control-Expose-Headers",
            "Content-Range, Content-Length, Accept-Ranges",
        )
        .header("Access-Control-Allow-Headers", "Range")
}

/// Handle OPTIONS preflight requests for the proxy endpoint.
pub(crate) async fn proxy_cors_preflight_handler() -> axum::http::Response<axum::body::Body> {
    apply_cors_headers(axum::http::Response::builder().status(204))
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .body(axum::body::Body::empty())
        .unwrap()
}

/// Serve cached audio from its on-disk file, streaming — never buffering the body.
/// Returns `200 OK` with the full body when there is no Range header, or `206 Partial
/// Content` with the requested byte slice when there is one. Returning `206`
/// unconditionally (even without a Range request) caused WKWebView's AVFoundation to
/// miscalculate the duration as ~2x the real value — the 200-vs-206 split must not change.
async fn serve_range_from_file(
    path: &Path,
    content_type: &str,
    total: u64,
    start: u64,
    end_opt: Option<u64>,
    is_range_request: bool,
) -> Result<axum::http::Response<axum::body::Body>> {
    use tokio::io::AsyncSeekExt;

    if is_range_request && start >= total {
        return Ok(
            apply_cors_headers(axum::http::Response::builder().status(416))
                .header("Content-Range", format!("bytes */{total}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        );
    }

    let mut file = tokio::fs::File::open(path).await?;

    if is_range_request {
        let end = end_opt.map_or(total - 1, |e| e.min(total - 1));
        let len = end - start + 1;
        file.seek(std::io::SeekFrom::Start(start)).await?;
        let limited = tokio::io::AsyncReadExt::take(file, len);
        let stream = tokio_util::io::ReaderStream::new(limited);

        Ok(
            apply_cors_headers(axum::http::Response::builder().status(206))
                .header("Content-Type", content_type)
                .header("Content-Range", format!("bytes {start}-{end}/{total}"))
                .header("Content-Length", len.to_string())
                .header("Accept-Ranges", "bytes")
                .body(axum::body::Body::from_stream(stream))
                .unwrap(),
        )
    } else {
        let stream = tokio_util::io::ReaderStream::new(file);
        Ok(
            apply_cors_headers(axum::http::Response::builder().status(200))
                .header("Content-Type", content_type)
                .header("Content-Length", total.to_string())
                .header("Accept-Ranges", "bytes")
                .body(axum::body::Body::from_stream(stream))
                .unwrap(),
        )
    }
}
