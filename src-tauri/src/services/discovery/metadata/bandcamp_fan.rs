use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{CrateError, Result};
use crate::models::ScrapedCollectionItem;

use super::common::decode_html_entities;

/// Paginated collection endpoint. POST `{fan_id, older_than_token, count}`, returns
/// `{items, more_available, last_token}`. Accepts `count` up to at least 100.
const FAN_COLLECTION_API_URL: &str = "https://bandcamp.com/api/fancollection/1/collection_items";

/// Root `bandcamp.com/<segment>` paths that are site pages, not fan profiles.
const RESERVED_FAN_PATHS: &[&str] = &[
    "about",
    "api",
    "apps",
    "artists",
    "band_follow",
    "concierge",
    "contact",
    "developer",
    "discover",
    "download",
    "embeddedplayer",
    "fan_follow",
    "feed",
    "gift_cards",
    "help",
    "jobs",
    "live",
    "login",
    "music",
    "privacy",
    "redeem",
    "search",
    "signup",
    "stats",
    "tag",
    "tags",
    "terms",
    "videoframe",
];

/// Resolve the link dialog's free-form input to a fan-page URL: a bare username
/// (optionally `@`-prefixed) becomes `https://bandcamp.com/<username>`; anything
/// URL-shaped passes through for the usual fan-URL checks. `None` for input that is
/// neither (spaces, dots outside a URL, empty). Usernames are lowercased — Bandcamp
/// assigns them lowercase, so this forgives a capitalized retype.
pub fn fan_input_to_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("://") || trimmed.to_lowercase().contains("bandcamp.com") {
        return Some(trimmed.to_string());
    }
    let handle = trimmed.strip_prefix('@').unwrap_or(trimmed);
    if !handle.is_empty()
        && handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Some(format!("https://bandcamp.com/{}", handle.to_lowercase()));
    }
    None
}

/// Returns `true` only for Bandcamp *fan* pages: `bandcamp.com/<username>` on the bare
/// domain (artist/label pages always live on `*.bandcamp.com` subdomains or custom
/// domains), with exactly one path segment that isn't a reserved site path.
pub fn is_bandcamp_fan_url(url: &str) -> bool {
    let lower = url.trim().to_lowercase();
    let rest = match lower.split_once("://") {
        Some(("http" | "https", rest)) => rest,
        Some(_) => return false,
        None => lower.as_str(),
    };
    let rest = match rest.find(&['?', '#'][..]) {
        Some(idx) => &rest[..idx],
        None => rest,
    };
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host, path),
        None => (rest, ""),
    };
    if host != "bandcamp.com" && host != "www.bandcamp.com" {
        return false;
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segments.len() == 1 && !RESERVED_FAN_PATHS.contains(&segments[0])
}

/// The fan's identity as read from the page — used to name the linked account.
#[derive(Debug, Clone, Serialize)]
pub struct FanProfile {
    pub fan_id: i64,
    pub username: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// The initial fan-page scrape: profile + first item batch + the pagination cursor.
#[derive(Debug, Clone)]
pub struct FanPage {
    pub profile: FanProfile,
    pub items: Vec<ScrapedCollectionItem>,
    pub last_token: Option<String>,
    pub item_count: Option<usize>,
}

/// One page from the collection API.
#[derive(Debug, Clone)]
pub struct FanCollectionBatch {
    pub items: Vec<ScrapedCollectionItem>,
    pub last_token: Option<String>,
    pub more_available: bool,
}

#[derive(Deserialize)]
struct PageData {
    fan_data: Option<RawFanData>,
    collection_data: Option<RawCollectionData>,
    item_cache: Option<RawItemCache>,
}

#[derive(Deserialize)]
struct RawFanData {
    fan_id: Option<i64>,
    username: Option<String>,
    name: Option<String>,
    photo: Option<RawFanPhoto>,
}

#[derive(Deserialize)]
struct RawFanPhoto {
    image_id: Option<i64>,
}

#[derive(Deserialize)]
struct RawCollectionData {
    item_count: Option<usize>,
    last_token: Option<String>,
    /// Item-cache keys (`"a<item_id>"` / `"t<item_id>"`) in display (newest-first) order.
    sequence: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawItemCache {
    collection: Option<HashMap<String, RawCollectionItem>>,
}

#[derive(Deserialize)]
struct RawCollectionItem {
    item_type: Option<String>,
    item_url: Option<String>,
    item_id: Option<i64>,
    band_name: Option<String>,
    item_title: Option<String>,
    item_art_id: Option<i64>,
    /// e.g. `"08 Jul 2026 10:02:53 GMT"`.
    purchased: Option<String>,
    is_private: Option<bool>,
}

#[derive(Deserialize)]
struct RawCollectionBatch {
    items: Option<Vec<RawCollectionItem>>,
    last_token: Option<String>,
    more_available: Option<bool>,
}

#[derive(Serialize)]
struct CollectionItemsRequest<'a> {
    fan_id: i64,
    older_than_token: &'a str,
    count: u32,
}

/// Fetch a fan page and parse its profile + first collection batch.
pub async fn fetch_fan_page(client: &reqwest::Client, url: &str) -> Result<FanPage> {
    let response =
        client.get(url).send().await.map_err(|e| {
            CrateError::Discovery(format!("Failed to fetch Bandcamp fan page: {e}"))
        })?;

    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(CrateError::Discovery(
            "Bandcamp rate limit exceeded (429)".into(),
        ));
    }
    if !response.status().is_success() {
        return Err(CrateError::Discovery(format!(
            "Bandcamp fan page returned status {}",
            response.status()
        )));
    }

    let html = response
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read Bandcamp response: {e}")))?;

    let blob = extract_pagedata_blob(&html).ok_or_else(|| {
        CrateError::Discovery("No collection data found — is this a Bandcamp fan page URL?".into())
    })?;
    let page = parse_fan_pagedata(&blob)?;

    if page.items.is_empty() && page.item_count.unwrap_or(0) == 0 {
        return Err(CrateError::Discovery(
            "This Bandcamp collection is private or empty".into(),
        ));
    }
    Ok(page)
}

/// Fetch one page of collection items past `older_than_token`.
pub async fn fetch_collection_batch(
    client: &reqwest::Client,
    fan_id: i64,
    older_than_token: &str,
    count: u32,
) -> Result<FanCollectionBatch> {
    let response = client
        .post(FAN_COLLECTION_API_URL)
        .json(&CollectionItemsRequest {
            fan_id,
            older_than_token,
            count,
        })
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch Bandcamp collection: {e}")))?;

    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(CrateError::Discovery(
            "Bandcamp rate limit exceeded (429)".into(),
        ));
    }
    if !response.status().is_success() {
        return Err(CrateError::Discovery(format!(
            "Bandcamp collection API returned status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read Bandcamp response: {e}")))?;
    parse_collection_batch(&body)
}

/// Extract the JSON string from the fan page's `<div id="pagedata" data-blob="...">`.
/// Entity-encoded like `data-client-items` — the first raw `"` closes the attribute.
pub(super) fn extract_pagedata_blob(html: &str) -> Option<String> {
    let anchor = html.find("id=\"pagedata\"")?;
    let marker = "data-blob=\"";
    let start = html[anchor..].find(marker)? + anchor + marker.len();
    let end = html[start..].find('"')? + start;
    let raw = &html[start..end];
    if raw.is_empty() {
        return None;
    }
    Some(decode_html_entities(raw))
}

pub(super) fn parse_fan_pagedata(json: &str) -> Result<FanPage> {
    let data: PageData = serde_json::from_str(json)
        .map_err(|e| CrateError::Discovery(format!("Failed to parse Bandcamp fan page: {e}")))?;

    let fan = data.fan_data.ok_or_else(|| {
        CrateError::Discovery("No fan profile found — is this a Bandcamp fan page URL?".into())
    })?;
    let fan_id = fan.fan_id.ok_or_else(|| {
        CrateError::Discovery("No fan profile found — is this a Bandcamp fan page URL?".into())
    })?;

    let profile = FanProfile {
        fan_id,
        username: fan.username.filter(|s| !s.is_empty()),
        name: fan.name.filter(|s| !s.is_empty()),
        avatar_url: fan
            .photo
            .and_then(|p| p.image_id)
            .map(|id| format!("https://f4.bcbits.com/img/{id}_42.jpg")),
    };

    let (item_count, last_token, sequence) = match data.collection_data {
        Some(cd) => (
            cd.item_count,
            cd.last_token,
            cd.sequence.unwrap_or_default(),
        ),
        None => (None, None, Vec::new()),
    };

    // item_cache.collection is keyed by "a<item_id>"/"t<item_id>"; `sequence` carries the
    // newest-first display order. Drain in that order, then append any stragglers.
    let mut cache = data
        .item_cache
        .and_then(|c| c.collection)
        .unwrap_or_default();
    let mut items = Vec::with_capacity(cache.len());
    for key in &sequence {
        if let Some(raw) = cache.remove(key) {
            if let Some(item) = convert_item(raw) {
                items.push(item);
            }
        }
    }
    let mut stragglers: Vec<(String, RawCollectionItem)> = cache.drain().collect();
    stragglers.sort_by(|(a, _), (b, _)| a.cmp(b));
    items.extend(
        stragglers
            .into_iter()
            .filter_map(|(_, raw)| convert_item(raw)),
    );

    Ok(FanPage {
        profile,
        items,
        last_token,
        item_count,
    })
}

pub(super) fn parse_collection_batch(json: &str) -> Result<FanCollectionBatch> {
    let batch: RawCollectionBatch = serde_json::from_str(json).map_err(|e| {
        CrateError::Discovery(format!("Failed to parse Bandcamp collection batch: {e}"))
    })?;
    Ok(FanCollectionBatch {
        items: batch
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(convert_item)
            .collect(),
        last_token: batch.last_token,
        more_available: batch.more_available.unwrap_or(false),
    })
}

/// Convert a raw scraped item, skipping private items and anything without a URL.
/// Unparseable individual items are dropped rather than failing the whole batch.
fn convert_item(raw: RawCollectionItem) -> Option<ScrapedCollectionItem> {
    if raw.is_private == Some(true) {
        return None;
    }
    let url = raw.item_url.filter(|u| !u.is_empty())?;
    let item_type = match raw.item_type.as_deref() {
        Some("track") => "track",
        // Bandcamp uses "album" but be permissive about unknown package-ish types:
        // anything with an /album/ URL is release-shaped.
        _ => "album",
    };
    Some(ScrapedCollectionItem {
        item_type: item_type.to_string(),
        url,
        external_id: raw.item_id.map(|id| id.to_string()),
        artist: raw.band_name.filter(|s| !s.is_empty()),
        title: raw.item_title.filter(|s| !s.is_empty()),
        artwork_url: raw
            .item_art_id
            .map(|id| format!("https://f4.bcbits.com/img/a{id}_16.jpg")),
        purchased_at: raw.purchased.as_deref().and_then(parse_purchased_date),
    })
}

/// Parse Bandcamp's `"08 Jul 2026 10:02:53 GMT"` purchase timestamps into RFC3339.
fn parse_purchased_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches(" GMT");
    chrono::NaiveDateTime::parse_from_str(trimmed, "%d %b %Y %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_input_accepts_usernames_and_urls() {
        // Bare usernames (with optional @, any case) become fan-page URLs.
        assert_eq!(
            fan_input_to_url("akatten").as_deref(),
            Some("https://bandcamp.com/akatten")
        );
        assert_eq!(
            fan_input_to_url(" @Akatten ").as_deref(),
            Some("https://bandcamp.com/akatten")
        );
        assert_eq!(
            fan_input_to_url("some_fan-99").as_deref(),
            Some("https://bandcamp.com/some_fan-99")
        );
        // URL-shaped input passes through untouched for the fan-URL checks.
        assert_eq!(
            fan_input_to_url("https://bandcamp.com/akatten").as_deref(),
            Some("https://bandcamp.com/akatten")
        );
        assert_eq!(
            fan_input_to_url("bandcamp.com/akatten").as_deref(),
            Some("bandcamp.com/akatten")
        );
        // Neither a handle nor a URL.
        assert_eq!(fan_input_to_url(""), None);
        assert_eq!(fan_input_to_url("   "), None);
        assert_eq!(fan_input_to_url("not a username"), None);
        assert_eq!(fan_input_to_url("example.com/foo"), None);
        assert_eq!(fan_input_to_url("@"), None);
    }

    #[test]
    fn fan_url_detection() {
        assert!(is_bandcamp_fan_url("https://bandcamp.com/akatten"));
        assert!(is_bandcamp_fan_url("https://www.bandcamp.com/akatten/"));
        assert!(is_bandcamp_fan_url("bandcamp.com/some_fan"));
        assert!(is_bandcamp_fan_url(
            "https://bandcamp.com/akatten?from=fanthanks"
        ));

        // Reserved site paths are not fan pages.
        assert!(!is_bandcamp_fan_url("https://bandcamp.com/discover"));
        assert!(!is_bandcamp_fan_url("https://bandcamp.com/tag/techno"));
        assert!(!is_bandcamp_fan_url("https://bandcamp.com/login"));
        // Artist pages live on subdomains / custom domains.
        assert!(!is_bandcamp_fan_url("https://aphextwin.bandcamp.com"));
        assert!(!is_bandcamp_fan_url("https://aphextwin.bandcamp.com/music"));
        // Release pages and the bare root are not fan pages.
        assert!(!is_bandcamp_fan_url("https://bandcamp.com"));
        assert!(!is_bandcamp_fan_url(
            "https://bandcamp.com/akatten/following"
        ));
        assert!(!is_bandcamp_fan_url("https://example.com/akatten"));
    }

    const FIXTURE_BLOB: &str = r#"{
        "fan_data": {
            "fan_id": 1941503,
            "username": "akatten",
            "name": "Akatten",
            "photo": {"image_id": 42512952, "width": 873, "height": 873}
        },
        "collection_data": {
            "item_count": 3,
            "batch_size": 20,
            "last_token": "1737882955:1966255171:a::",
            "sequence": ["a729464671", "t555", "a111"]
        },
        "item_cache": {
            "collection": {
                "a111": {
                    "item_type": "album",
                    "item_url": "https://label.example.com/album/other",
                    "item_id": 111,
                    "band_name": "Custom Domain Band",
                    "item_title": "Other",
                    "item_art_id": 42,
                    "purchased": "01 Jan 2025 00:00:00 GMT"
                },
                "a729464671": {
                    "item_type": "album",
                    "item_url": "https://puzzle2.bandcamp.com/album/x-hail",
                    "item_id": 729464671,
                    "band_name": "PUZZLE",
                    "item_title": "X Hail",
                    "item_art_id": 3885046849,
                    "purchased": "08 Jul 2026 10:02:53 GMT"
                },
                "t555": {
                    "item_type": "track",
                    "item_url": "https://artist.bandcamp.com/track/single",
                    "item_id": 555,
                    "band_name": "Artist",
                    "item_title": "Single",
                    "item_art_id": 7,
                    "purchased": "bogus date",
                    "is_private": false
                },
                "a999": {
                    "item_type": "album",
                    "item_url": "https://hidden.bandcamp.com/album/secret",
                    "item_id": 999,
                    "is_private": true
                }
            }
        }
    }"#;

    #[test]
    fn parses_fan_pagedata_in_sequence_order() {
        let page = parse_fan_pagedata(FIXTURE_BLOB).unwrap();

        assert_eq!(page.profile.fan_id, 1941503);
        assert_eq!(page.profile.username.as_deref(), Some("akatten"));
        assert_eq!(page.profile.name.as_deref(), Some("Akatten"));
        assert_eq!(
            page.profile.avatar_url.as_deref(),
            Some("https://f4.bcbits.com/img/42512952_42.jpg")
        );
        assert_eq!(page.item_count, Some(3));
        assert_eq!(
            page.last_token.as_deref(),
            Some("1737882955:1966255171:a::")
        );

        // Private item dropped; the rest follow `sequence` order.
        assert_eq!(page.items.len(), 3);
        assert_eq!(page.items[0].title.as_deref(), Some("X Hail"));
        assert_eq!(page.items[1].item_type, "track");
        assert_eq!(page.items[2].artist.as_deref(), Some("Custom Domain Band"));

        assert_eq!(
            page.items[0].artwork_url.as_deref(),
            Some("https://f4.bcbits.com/img/a3885046849_16.jpg")
        );
        assert_eq!(
            page.items[0].purchased_at.as_deref(),
            Some("2026-07-08T10:02:53+00:00")
        );
        // Unparseable purchase dates degrade to None instead of dropping the item.
        assert_eq!(page.items[1].purchased_at, None);
        assert_eq!(page.items[0].external_id.as_deref(), Some("729464671"));
    }

    #[test]
    fn rejects_non_fan_pagedata() {
        assert!(parse_fan_pagedata(r#"{"cfg": {}}"#).is_err());
        assert!(parse_fan_pagedata("not json").is_err());
    }

    #[test]
    fn extracts_pagedata_blob_from_html() {
        let html =
            r#"<html><div id="pagedata" data-blob="{&quot;fan_data&quot;: null}"></div></html>"#;
        assert_eq!(
            extract_pagedata_blob(html).as_deref(),
            Some(r#"{"fan_data": null}"#)
        );
        assert_eq!(extract_pagedata_blob("<html></html>"), None);
    }

    #[test]
    fn parses_collection_batch() {
        let json = r#"{
            "items": [
                {"item_type": "album", "item_url": "https://a.bandcamp.com/album/x", "item_id": 1},
                {"item_type": "track", "item_url": "", "item_id": 2}
            ],
            "more_available": true,
            "last_token": "1669834876:856384793:t::"
        }"#;
        let batch = parse_collection_batch(json).unwrap();
        // Empty-URL item dropped.
        assert_eq!(batch.items.len(), 1);
        assert!(batch.more_available);
        assert_eq!(
            batch.last_token.as_deref(),
            Some("1669834876:856384793:t::")
        );
    }

    /// Scrape-fragility canary: run manually against a freshly saved fan page with
    /// `BANDCAMP_FAN_HTML=/path/to/page.html cargo test --features desktop real_fan_page -- --ignored`
    #[test]
    #[ignore]
    fn real_fan_page_html_roundtrip() {
        let path = std::env::var("BANDCAMP_FAN_HTML")
            .expect("set BANDCAMP_FAN_HTML to a saved fan-page html file");
        let html = std::fs::read_to_string(path).unwrap();
        let blob = extract_pagedata_blob(&html).expect("pagedata blob not found");
        let page = parse_fan_pagedata(&blob).unwrap();
        assert!(page.profile.fan_id > 0);
        assert!(!page.items.is_empty());
        assert!(page.last_token.is_some());
        assert!(page.items.iter().all(|i| i.url.starts_with("http")));
    }

    #[test]
    fn parses_purchased_dates() {
        assert_eq!(
            parse_purchased_date("08 Jul 2026 10:02:53 GMT").as_deref(),
            Some("2026-07-08T10:02:53+00:00")
        );
        assert_eq!(parse_purchased_date("not a date"), None);
    }
}
