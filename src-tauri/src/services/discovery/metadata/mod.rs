mod bandcamp;
mod common;
mod discogs;
mod soundcloud;
mod youtube;

#[cfg(test)]
mod tests;

use serde::Serialize;

use crate::error::{CrateError, Result};

use super::detect_source_type;

/// Chrome User-Agent shared across all YouTube-facing HTTP clients.
pub(super) const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// SOCS consent cookie that bypasses YouTube's EU cookie consent wall.
pub(super) const YT_CONSENT_COOKIE: &str = "SOCS=CAISNJAgJB";

#[derive(Debug, Clone, Serialize)]
pub struct FetchedMetadata {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
    pub tracks: Vec<FetchedTrack>,
    pub source_type: String,
    pub parent_url: Option<String>,
    pub parent_album_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchedTrack {
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
    pub video_id: Option<String>,
}

pub(super) fn is_compilation(artist: &Option<String>) -> bool {
    let Some(artist) = artist else { return false };
    let normalized = artist.trim().to_lowercase();
    matches!(
        normalized.as_str(),
        "various artists" | "various" | "v.a." | "v/a" | "va"
    )
}

pub(super) fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(CHROME_USER_AGENT)
        .build()
        .map_err(|e| CrateError::Discovery(format!("Failed to create HTTP client: {e}")))
}

pub async fn fetch_metadata(url: &str) -> Result<FetchedMetadata> {
    let client = build_client()?;
    let source_type = detect_source_type(url);

    let mut metadata = match source_type.as_str() {
        "bandcamp" => bandcamp::fetch_bandcamp(&client, url).await,
        "soundcloud" => soundcloud::fetch_soundcloud(&client, url).await,
        "youtube" => youtube::fetch_youtube(&client, url).await,
        "discogs" => discogs::fetch_discogs(&client, url).await,
        _ => return Err(CrateError::Discovery("Unsupported URL domain".into())),
    }?;

    metadata.source_type = source_type;
    metadata.release_date = metadata.release_date.map(|d| common::normalize_date(&d));
    Ok(metadata)
}

/// Scan an artist/label page URL and return all releases found on it.
/// Fetch just the profile/avatar image (og:image) for an artist/label page, without
/// scanning its releases — for previewing a follow target before following.
pub async fn fetch_page_avatar(url: &str) -> Result<Option<String>> {
    let client = build_client()?;
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch page: {e}")))?
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read page: {e}")))?;
    Ok(common::extract_meta_content(&html, "og:image"))
}

pub async fn scan_page(
    url: &str,
    existing_urls: &std::collections::HashSet<String>,
    cancel_flag: &std::sync::atomic::AtomicBool,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<crate::models::ScannedPage> {
    log::info!("Starting page scan for URL: {url}");
    let client = build_client()?;

    if bandcamp::is_bandcamp_page_url(url) {
        let (mut releases, page_name, avatar_url) =
            bandcamp::scan_bandcamp_page(&client, url).await?;

        // Normalize URLs and check existing
        let mut already_in_discovery = 0;
        for r in &mut releases {
            r.url = super::normalize_url(&r.url);
            r.already_exists = existing_urls.contains(&r.url);
            if r.already_exists {
                already_in_discovery += 1;
            }
        }

        let total_found = releases.len();

        return Ok(crate::models::ScannedPage {
            source_type: "bandcamp".to_string(),
            page_url: super::followable_page_url(url, "bandcamp"),
            page_artist: page_name.clone(),
            page_label: page_name,
            avatar_url,
            total_found,
            already_in_discovery,
            releases,
        });
    }

    if soundcloud::is_soundcloud_page_url(url) {
        let (mut releases, page_name, avatar_url) =
            soundcloud::scan_soundcloud_page(&client, url).await?;

        let mut already_in_discovery = 0;
        for r in &mut releases {
            r.url = super::normalize_url(&r.url);
            r.already_exists = existing_urls.contains(&r.url);
            if r.already_exists {
                already_in_discovery += 1;
            }
        }

        let total_found = releases.len();

        return Ok(crate::models::ScannedPage {
            source_type: "soundcloud".to_string(),
            page_url: super::followable_page_url(url, "soundcloud"),
            page_artist: page_name.clone(),
            page_label: page_name,
            avatar_url,
            total_found,
            already_in_discovery,
            releases,
        });
    }

    if let Some(kind) = discogs::parse_discogs_url(url) {
        if matches!(
            kind,
            discogs::DiscogsUrlKind::Artist(_) | discogs::DiscogsUrlKind::Label(_)
        ) {
            let (mut releases, page_artist, page_label, avatar_url) =
                discogs::scan_discogs_page(&client, &kind, cancel_flag, app_handle).await?;

            let mut already_in_discovery = 0;
            for r in &mut releases {
                r.url = super::normalize_url(&r.url);
                r.already_exists = existing_urls.contains(&r.url);
                if r.already_exists {
                    already_in_discovery += 1;
                }
            }

            let total_found = releases.len();

            return Ok(crate::models::ScannedPage {
                source_type: "discogs".to_string(),
                page_url: super::followable_page_url(url, "discogs"),
                page_artist,
                page_label,
                avatar_url,
                total_found,
                already_in_discovery,
                releases,
            });
        }
    }

    Err(CrateError::Discovery(
        "URL is not a supported artist or label page".into(),
    ))
}

// Re-exports for streams.rs, n_transform.rs, and commands/discovery.rs
pub(crate) use youtube::{
    build_yt_client_with_config, extract_playlist_videos, extract_query_param,
    fetch_yt_player_response_with_config, jittered_delay, new_yt_cookie_jar, parse_youtube_url,
    parse_yt_initial_data, YT_CLIENTS,
};
