//! Live canaries for the third-party discovery sources whose protocols change under us.
//!
//! These hit the real YouTube and Discogs endpoints, so they are `#[ignore]`d and run on
//! demand or from the scheduled `ci.canary.yml` workflow:
//!
//! ```sh
//! cd src-tauri && cargo test --features desktop -- --ignored canary_
//! ```
//!
//! A failure whose message starts with `INCONCLUSIVE` is YouTube's bot check rejecting the
//! caller's IP (common for datacenter ranges) rather than a protocol change; the workflow
//! reports those as warnings instead of opening an issue.

use super::discogs::fetch_discogs;
use super::youtube::{extract_playlist_videos, parse_playlist_header, parse_yt_initial_data};
use super::{build_client, YT_CONSENT_COOKIE};
use crate::services::discovery::streams::extract_single_youtube_stream;

/// Stable, non-age-gated, non-kids uploads; the last two are what Discogs release 249504 links.
const CANARY_VIDEO_IDS: &[&str] = &["dQw4w9WgXcQ", "7FwDP17XPlk", "hTWKbfoikeg"];
/// A 13-video artist playlist (Rick Astley - 50 (Album)).
const CANARY_PLAYLIST_ID: &str = "PLlaN88a7y2_oBUxLd3j23dkAFNtM-P24e";
/// Rick Astley - Never Gonna Give You Up, which carries a long `videos[]` list.
const CANARY_DISCOGS_RELEASE: &str = "https://www.discogs.com/release/249504";

fn is_bot_check(err: &str) -> bool {
    err.contains("LOGIN_REQUIRED") || err.contains("not a bot")
}

#[tokio::test]
#[ignore]
async fn canary_youtube_stream_extraction() {
    let mut failures = Vec::new();
    let mut bot_checks = Vec::new();

    for (idx, video_id) in CANARY_VIDEO_IDS.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        match extract_single_youtube_stream(video_id, 1).await {
            Ok(stream) => {
                let client = build_client().expect("client");
                let mut req = client
                    .get(&stream.stream_url)
                    .header("Range", "bytes=0-1023");
                if let Some(ua) = &stream.proxy_ua {
                    req = req.header("User-Agent", ua);
                }
                match req.send().await {
                    Ok(resp) if resp.status().as_u16() == 206 => {
                        let ct = resp
                            .headers()
                            .get("Content-Type")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or_default()
                            .to_string();
                        if !ct.starts_with("audio/") {
                            failures.push(format!("{video_id}: CDN served content-type {ct:?}"));
                        }
                    }
                    Ok(resp) => failures.push(format!(
                        "{video_id}: CDN range request answered {}",
                        resp.status()
                    )),
                    Err(e) => failures.push(format!("{video_id}: CDN request failed: {e}")),
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if is_bot_check(&msg) {
                    bot_checks.push(format!("{video_id}: {msg}"));
                } else {
                    failures.push(format!("{video_id}: {msg}"));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "YouTube stream extraction broke (protocol change?):\n{}",
        failures.join("\n")
    );
    assert!(
        bot_checks.len() < CANARY_VIDEO_IDS.len(),
        "INCONCLUSIVE: every video hit YouTube's bot check from this IP:\n{}",
        bot_checks.join("\n")
    );
}

#[tokio::test]
#[ignore]
async fn canary_youtube_playlist_page() {
    let client = build_client().expect("client");
    let html = client
        .get(format!(
            "https://www.youtube.com/playlist?list={CANARY_PLAYLIST_ID}"
        ))
        .header("Cookie", YT_CONSENT_COOKIE)
        .send()
        .await
        .expect("playlist page request")
        .text()
        .await
        .expect("playlist page body");

    let yt_data = parse_yt_initial_data(&html).expect("ytInitialData missing from playlist page");
    let (title, owner) = parse_playlist_header(&yt_data);
    assert!(
        title.as_deref().is_some_and(|t| !t.is_empty()),
        "playlist header title not found (header shape changed?)"
    );
    assert_eq!(
        owner.as_deref(),
        Some("Rick Astley"),
        "playlist owner not found"
    );

    // The page server-renders the playlist's available videos (6 of this album's 13 entries
    // are region-hidden), so the bar is "several parsed", not the full count.
    let videos = extract_playlist_videos(&yt_data);
    assert!(
        videos.len() >= 5,
        "expected >= 5 playlist videos, parsed {} (item renderer changed?)",
        videos.len()
    );
    assert!(
        videos
            .iter()
            .all(|v| v.title != "Untitled" && v.duration_ms.is_some()),
        "some playlist videos lost their title or duration: {videos:?}"
    );
}

#[tokio::test]
#[ignore]
async fn canary_discogs_release_videos() {
    let client = build_client().expect("client");
    let fetched = fetch_discogs(&client, CANARY_DISCOGS_RELEASE)
        .await
        .expect("Discogs release fetch");
    assert_eq!(fetched.artist.as_deref(), Some("Rick Astley"));
    assert!(
        !fetched.tracks.is_empty(),
        "no tracks parsed from Discogs release"
    );
    assert!(
        fetched.tracks.iter().any(|t| t.video_id.is_some()),
        "no track received a YouTube video id from Discogs videos[]"
    );
}
