use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{CrateError, Result};
use crate::services::DiscoveryService;

use tauri::Manager;

/// Size of each sequential download chunk (~1 MB).
/// YouTube CDN allows exactly one ~1 MB request per video per IP for IOS client URLs.
const CHUNK_SIZE: u64 = 1_048_576;

/// Maximum number of cached audio entries kept in memory.
const MAX_CACHE_ENTRIES: usize = 3;

/// Maximum total download size to prevent runaway downloads (~20 MB).
const MAX_TOTAL_SIZE: u64 = 20_971_520;

/// Fully downloaded audio file held in memory for instant range serving.
struct CachedAudio {
    data: Vec<u8>,
    content_type: String,
}

/// Shared state threaded into every axum proxy request handler.
#[derive(Clone)]
pub(crate) struct ProxyServerState {
    pub app_handle: tauri::AppHandle,
    pub client: reqwest::Client,
    /// Cache of fully downloaded audio files, keyed by "{release_id}/{track_position}".
    cache: Arc<tokio::sync::RwLock<HashMap<String, Arc<CachedAudio>>>>,
    /// In-flight downloads: maps cache key to a watch receiver that signals completion.
    /// `None` = download in progress, `Some(true)` = success, `Some(false)` = failed.
    downloads: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::watch::Receiver<Option<bool>>>>>,
}

impl ProxyServerState {
    pub fn new(app_handle: tauri::AppHandle, client: reqwest::Client) -> Self {
        Self {
            app_handle,
            client,
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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
    cr.split('/').last()?.parse().ok()
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
    let (start, end_opt) = parse_bytes_range(incoming_range);

    // 1. Check cache — if audio data is available, serve from memory.
    {
        let cache = state.cache.read().await;
        if let Some(cached) = cache.get(&cache_key) {
            return serve_range_from_cache(cached, start, end_opt);
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

            let proxy_ua = cached_stream.proxy_ua.ok_or_else(|| {
                CrateError::Discovery(
                    "Cached stream has no proxy_ua — should use direct URL".into(),
                )
            })?;

            let (tx, rx) = tokio::sync::watch::channel(None);
            downloads.insert(cache_key.clone(), rx.clone());

            let state_clone = state.clone();
            let url = cached_stream.stream_url.clone();
            let key = cache_key.clone();

            tokio::spawn(async move {
                let success = match download_stream(&state_clone, &url, &proxy_ua).await {
                    Ok(cached_audio) => {
                        let entry = Arc::new(cached_audio);
                        let mut cache = state_clone.cache.write().await;

                        while cache.len() >= MAX_CACHE_ENTRIES {
                            if let Some(oldest_key) = cache.keys().next().cloned() {
                                cache.remove(&oldest_key);
                            }
                        }

                        cache.insert(key.clone(), entry);
                        true
                    }
                    Err(e) => {
                        log::warn!("Stream download failed for {key}: {e}");
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

    // 4. Serve from cache.
    let cache = state.cache.read().await;
    let cached = cache.get(&cache_key).ok_or_else(|| {
        CrateError::Discovery("Download succeeded but cache entry missing".into())
    })?;
    serve_range_from_cache(cached, start, end_opt)
}

/// Download an audio stream in sequential chunks.
///
/// Downloads as many 1 MB chunks as the CDN allows. YouTube CDN enforces a per-video per-IP
/// rate limit for IOS client URLs (typically one request), so this may produce a partial cache.
/// Browser-compatible clients (WEB, WEB_EMBEDDED) don't go through the proxy at all.
async fn download_stream(
    state: &ProxyServerState,
    stream_url: &str,
    proxy_ua: &str,
) -> Result<CachedAudio> {
    let mut data = Vec::new();
    let mut content_type = String::from("audio/mp4");
    let mut total_size: Option<u64> = None;

    loop {
        let offset = data.len() as u64;

        if let Some(total) = total_size {
            if offset >= total {
                break;
            }
        }
        if offset >= MAX_TOTAL_SIZE {
            break;
        }

        let range_end = offset + CHUNK_SIZE - 1;
        let response = state
            .client
            .get(stream_url)
            .header("User-Agent", proxy_ua)
            .header("Range", format!("bytes={offset}-{range_end}"))
            .send()
            .await
            .map_err(|e| CrateError::Discovery(format!("Stream chunk request failed: {e:#}")))?;

        let status = response.status();
        if !status.is_success() && status.as_u16() != 206 {
            if data.is_empty() {
                return Err(CrateError::Discovery(format!(
                    "Stream download returned {status}"
                )));
            }
            // CDN rejected subsequent chunk — cache what we have.
            log::info!(
                "CDN rejected chunk at offset {offset} ({status}), caching {} bytes of partial audio",
                data.len()
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

        data.extend_from_slice(&chunk_bytes);
    }

    if data.is_empty() {
        return Err(CrateError::Discovery(
            "Stream download produced no data".into(),
        ));
    }

    let full = total_size.map_or(true, |total| data.len() as u64 >= total);
    log::info!(
        "Stream download {}: {} bytes cached, content-type: {content_type}",
        if full { "complete" } else { "partial" },
        data.len()
    );

    Ok(CachedAudio { data, content_type })
}

/// Serve a byte range from cached audio data.
fn serve_range_from_cache(
    cached: &CachedAudio,
    start: u64,
    end_opt: Option<u64>,
) -> Result<axum::http::Response<axum::body::Body>> {
    let total = cached.data.len() as u64;

    if start >= total {
        return Ok(axum::http::Response::builder()
            .status(416)
            .header("Content-Range", format!("bytes */{total}"))
            .body(axum::body::Body::empty())
            .unwrap());
    }

    let end = end_opt.map_or(total - 1, |e| e.min(total - 1));
    let slice = &cached.data[start as usize..=end as usize];

    Ok(axum::http::Response::builder()
        .status(206)
        .header("Content-Type", &cached.content_type)
        .header("Content-Range", format!("bytes {start}-{end}/{total}"))
        .header("Content-Length", slice.len().to_string())
        .header("Accept-Ranges", "bytes")
        .header("Access-Control-Allow-Origin", "*")
        .body(axum::body::Body::from(slice.to_vec()))
        .unwrap())
}
