use super::bandcamp::parse_bandcamp_json_ld;
use super::common::{extract_meta_content, normalize_date, parse_iso_duration};
use super::discogs::{
    join_discogs_artists, parse_discogs_duration, parse_discogs_url, score_discogs_release,
    strip_discogs_suffix, DiscogsUrlKind,
};
use super::soundcloud::parse_sc_hydration;
use super::youtube::{
    extract_playlist_videos, extract_query_param, parse_youtube_url, parse_yt_initial_data,
};

#[test]
fn test_normalize_date_iso_format() {
    assert_eq!(normalize_date("2024-06-19"), "2024-06-19");
    assert_eq!(normalize_date("2025-05-08T00:00:00Z"), "2025-05-08");
    assert_eq!(normalize_date("2024-06-27T00:00:00Z"), "2024-06-27");
}

#[test]
fn test_normalize_date_bandcamp_format() {
    assert_eq!(normalize_date("19 Jun 2024 00:00:00 GMT"), "2024-06-19");
    assert_eq!(normalize_date("01 Jan 2025 00:00:00 GMT"), "2025-01-01");
    assert_eq!(normalize_date("31 Dec 2023 00:00:00 GMT"), "2023-12-31");
}

#[test]
fn test_normalize_date_passthrough() {
    assert_eq!(normalize_date("unknown"), "unknown");
    assert_eq!(normalize_date(""), "");
}

#[test]
fn test_parse_iso_duration() {
    assert_eq!(parse_iso_duration("PT3M45S"), Some(225_000));
    assert_eq!(parse_iso_duration("PT1H2M3S"), Some(3_723_000));
    assert_eq!(parse_iso_duration("PT30S"), Some(30_000));
    assert_eq!(parse_iso_duration("PT5M"), Some(300_000));
    assert_eq!(parse_iso_duration("invalid"), None);
    // Bandcamp format: P prefix without T separator
    assert_eq!(parse_iso_duration("P00H03M45S"), Some(225_000));
    assert_eq!(parse_iso_duration("P00H06M12S"), Some(372_000));
    assert_eq!(parse_iso_duration("P01H02M03S"), Some(3_723_000));
}

#[test]
fn test_extract_meta_content() {
    let html = r#"<html><head><meta property="og:title" content="Test Title"><meta property="og:image" content="https://example.com/img.jpg"></head></html>"#;
    assert_eq!(
        extract_meta_content(html, "og:title"),
        Some("Test Title".to_string())
    );
    assert_eq!(
        extract_meta_content(html, "og:image"),
        Some("https://example.com/img.jpg".to_string())
    );
    assert_eq!(extract_meta_content(html, "og:description"), None);
}

fn make_sc_hydration_html(hydration_json: &str) -> String {
    format!(
        "<html><head></head><body><script>window.__sc_hydration = {hydration_json};</script></body></html>"
    )
}

#[test]
fn test_parse_sc_hydration() {
    let html = make_sc_hydration_html(
        r#"[
                {"hydratable": "user", "data": {}},
                {"hydratable": "sound", "data": {
                    "title": "Mind Fog",
                    "duration": 345000,
                    "artwork_url": "https://i1.sndcdn.com/artworks-abc-large.jpg",
                    "release_date": "2025-05-08T00:00:00Z",
                    "user": {"username": "Apellum"},
                    "publisher_metadata": {
                        "artist": "Apellum, Gansi",
                        "publisher": "Some Label"
                    }
                }}
            ]"#,
    );
    let meta = parse_sc_hydration(&html).expect("should parse hydration");
    assert_eq!(meta.title.as_deref(), Some("Mind Fog"));
    assert_eq!(meta.artist.as_deref(), Some("Apellum, Gansi"));
    assert_eq!(meta.label.as_deref(), Some("Some Label"));
    assert_eq!(meta.release_date.as_deref(), Some("2025-05-08"));
    assert_eq!(
        meta.artwork_url.as_deref(),
        Some("https://i1.sndcdn.com/artworks-abc-t500x500.jpg")
    );
    assert_eq!(meta.tracks.len(), 1);
    assert_eq!(meta.tracks[0].duration_ms, Some(345000));
}

#[test]
fn test_parse_sc_hydration_fallback_to_user() {
    let html = make_sc_hydration_html(
        r#"[
                {"hydratable": "sound", "data": {
                    "title": "Some Track",
                    "duration": 200000,
                    "user": {"username": "DJ Test"},
                    "publisher_metadata": {}
                }}
            ]"#,
    );
    let meta = parse_sc_hydration(&html).expect("should parse hydration");
    assert_eq!(meta.artist.as_deref(), Some("DJ Test"));
    assert_eq!(meta.label, None);
    assert_eq!(meta.release_date, None);
}

#[test]
fn test_parse_sc_hydration_label_upload() {
    let html = make_sc_hydration_html(
        r#"[
                {"hydratable": "sound", "data": {
                    "title": "Apellum - Sunshower",
                    "duration": 324046,
                    "artwork_url": "https://i1.sndcdn.com/artworks-xyz-large.jpg",
                    "release_date": "2024-06-27T00:00:00Z",
                    "label_name": "Perfect Dark",
                    "user": {"username": "Perfect Dark"},
                    "publisher_metadata": {
                        "artist": "Apellum"
                    }
                }}
            ]"#,
    );
    let meta = parse_sc_hydration(&html).expect("should parse hydration");
    assert_eq!(meta.title.as_deref(), Some("Sunshower"));
    assert_eq!(meta.artist.as_deref(), Some("Apellum"));
    assert_eq!(meta.label.as_deref(), Some("Perfect Dark"));
    assert_eq!(meta.release_date.as_deref(), Some("2024-06-27"));
}

#[test]
fn test_parse_sc_hydration_no_sound() {
    let html = make_sc_hydration_html(r#"[{"hydratable": "user", "data": {}}]"#);
    assert!(parse_sc_hydration(&html).is_none());
}

fn make_bandcamp_json_ld_html(json_ld: &str) -> String {
    format!(
        r#"<html><head><script type="application/ld+json">{json_ld}</script></head><body></body></html>"#
    )
}

#[test]
fn test_bandcamp_music_recording_creates_single_track() {
    let html = make_bandcamp_json_ld_html(
        r#"{
                "@type": "MusicRecording",
                "name": "Echoes",
                "duration": "P00H04M30S",
                "byArtist": {"name": "Test Artist"},
                "image": "https://example.com/art.jpg"
            }"#,
    );
    let meta = parse_bandcamp_json_ld(&html).expect("should parse MusicRecording");
    assert_eq!(meta.title.as_deref(), Some("Echoes"));
    assert_eq!(meta.tracks.len(), 1);
    assert_eq!(meta.tracks[0].name, "Echoes");
    assert_eq!(meta.tracks[0].position, 1);
    assert_eq!(meta.tracks[0].duration_ms, Some(270_000));
}

#[test]
fn test_bandcamp_music_recording_without_duration() {
    let html = make_bandcamp_json_ld_html(
        r#"{
                "@type": "MusicRecording",
                "name": "No Duration Track",
                "byArtist": {"name": "Test Artist"}
            }"#,
    );
    let meta = parse_bandcamp_json_ld(&html).expect("should parse MusicRecording");
    assert_eq!(meta.tracks.len(), 1);
    assert_eq!(meta.tracks[0].name, "No Duration Track");
    assert_eq!(meta.tracks[0].duration_ms, None);
}

#[test]
fn test_bandcamp_type_as_array() {
    let html = make_bandcamp_json_ld_html(
        r#"{
                "@type": ["MusicAlbum", "MusicRelease"],
                "name": "Array Type Album",
                "byArtist": {"name": "Test Artist"},
                "track": {
                    "itemListElement": [
                        {"position": 1, "item": {"name": "Track One", "duration": "PT3M00S"}}
                    ]
                }
            }"#,
    );
    let meta = parse_bandcamp_json_ld(&html).expect("should parse array @type");
    assert_eq!(meta.title.as_deref(), Some("Array Type Album"));
    assert_eq!(meta.tracks.len(), 1);
    assert_eq!(meta.tracks[0].name, "Track One");
    assert_eq!(meta.tracks[0].duration_ms, Some(180_000));
}

// =========================================================================
// Discogs helpers
// =========================================================================

#[test]
fn test_parse_discogs_url() {
    assert_eq!(
        parse_discogs_url("https://www.discogs.com/release/12345-Some-Slug"),
        Some(DiscogsUrlKind::Release(12345))
    );
    assert_eq!(
        parse_discogs_url("https://www.discogs.com/master/67890-Some-Slug"),
        Some(DiscogsUrlKind::Master(67890))
    );
    // Old format with artist prefix
    assert_eq!(
        parse_discogs_url("https://www.discogs.com/Artist-Name/release/99999"),
        Some(DiscogsUrlKind::Release(99999))
    );
    // No slug
    assert_eq!(
        parse_discogs_url("https://www.discogs.com/release/42"),
        Some(DiscogsUrlKind::Release(42))
    );
    // With query params
    assert_eq!(
        parse_discogs_url("https://www.discogs.com/release/123?anv=foo"),
        Some(DiscogsUrlKind::Release(123))
    );
    // Artist/label pages (bulk import)
    assert_eq!(
        parse_discogs_url("https://www.discogs.com/artist/12345"),
        Some(DiscogsUrlKind::Artist(12345))
    );
    assert_eq!(parse_discogs_url("https://example.com/release/abc"), None);
}

#[test]
fn test_parse_discogs_duration() {
    assert_eq!(parse_discogs_duration("4:30"), Some(270_000));
    assert_eq!(parse_discogs_duration("0:45"), Some(45_000));
    assert_eq!(parse_discogs_duration("1:02:15"), Some(3_735_000));
    assert_eq!(parse_discogs_duration(""), None);
    assert_eq!(parse_discogs_duration("  "), None);
    assert_eq!(parse_discogs_duration("invalid"), None);
    assert_eq!(parse_discogs_duration("0:00"), None);
}

#[test]
fn test_strip_discogs_suffix() {
    assert_eq!(strip_discogs_suffix("Artist (2)"), "Artist");
    assert_eq!(strip_discogs_suffix("Artist (15)"), "Artist");
    // Non-numeric suffixes are preserved
    assert_eq!(strip_discogs_suffix("Artist (UK)"), "Artist (UK)");
    // No suffix
    assert_eq!(strip_discogs_suffix("Regular Artist"), "Regular Artist");
}

#[test]
fn test_join_discogs_artists() {
    // Single artist
    let single = vec![serde_json::json!({"name": "Aphex Twin"})];
    assert_eq!(
        join_discogs_artists(&single),
        Some("Aphex Twin".to_string())
    );

    // Multi with comma join
    let multi = vec![
        serde_json::json!({"name": "Artist A", "join": ","}),
        serde_json::json!({"name": "Artist B"}),
    ];
    assert_eq!(
        join_discogs_artists(&multi),
        Some("Artist A, Artist B".to_string())
    );

    // Multi with ampersand join
    let ampersand = vec![
        serde_json::json!({"name": "Artist A", "join": "&"}),
        serde_json::json!({"name": "Artist B"}),
    ];
    assert_eq!(
        join_discogs_artists(&ampersand),
        Some("Artist A & Artist B".to_string())
    );

    // Strips disambiguation suffixes
    let disambig = vec![serde_json::json!({"name": "Artist (2)"})];
    assert_eq!(join_discogs_artists(&disambig), Some("Artist".to_string()));

    // Empty
    let empty: Vec<serde_json::Value> = vec![];
    assert_eq!(join_discogs_artists(&empty), None);
}

#[test]
fn test_score_discogs_release_prefers_artwork() {
    let with_art = serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg"});
    let without_art = serde_json::json!({"thumb": ""});
    let no_thumb = serde_json::json!({});
    assert!(score_discogs_release(&with_art, 100) > score_discogs_release(&without_art, 100));
    assert!(score_discogs_release(&with_art, 100) > score_discogs_release(&no_thumb, 100));
}

#[test]
fn test_score_discogs_release_penalizes_test_pressing() {
    let official =
        serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\", EP"});
    let test_press = serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\", EP, Test Pressing"});
    assert!(score_discogs_release(&official, 100) > score_discogs_release(&test_press, 100));
}

#[test]
fn test_score_discogs_release_penalizes_promo() {
    let official =
        serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\""});
    let promo =
        serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\", Promo"});
    assert!(score_discogs_release(&official, 100) > score_discogs_release(&promo, 100));
}

#[test]
fn test_score_discogs_release_penalizes_white_label() {
    let official =
        serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\""});
    let white_label = serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\", White Label"});
    assert!(score_discogs_release(&official, 100) > score_discogs_release(&white_label, 100));
}

#[test]
fn test_score_discogs_release_artwork_beats_format_penalty() {
    // A test pressing with artwork should still score higher than an official release without
    let test_with_art = serde_json::json!({"thumb": "https://img.discogs.com/abc.jpg", "format": "12\", Test Pressing"});
    let official_no_art = serde_json::json!({"thumb": "", "format": "12\""});
    assert!(
        score_discogs_release(&test_with_art, 100) > score_discogs_release(&official_no_art, 100)
    );
}

// =========================================================================
// YouTube helpers
// =========================================================================

#[test]
fn test_parse_youtube_url_standard() {
    let parsed = parse_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    assert_eq!(parsed.video_id.as_deref(), Some("dQw4w9WgXcQ"));
    assert_eq!(parsed.playlist_id, None);
}

#[test]
fn test_parse_youtube_url_with_playlist() {
    let parsed = parse_youtube_url("https://www.youtube.com/watch?v=abc123&list=PLxyz789");
    assert_eq!(parsed.video_id.as_deref(), Some("abc123"));
    assert_eq!(parsed.playlist_id.as_deref(), Some("PLxyz789"));
}

#[test]
fn test_parse_youtube_url_playlist_only() {
    let parsed = parse_youtube_url("https://www.youtube.com/playlist?list=PLxyz789");
    assert_eq!(parsed.video_id, None);
    assert_eq!(parsed.playlist_id.as_deref(), Some("PLxyz789"));
}

#[test]
fn test_parse_youtube_url_short() {
    let parsed = parse_youtube_url("https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(parsed.video_id.as_deref(), Some("dQw4w9WgXcQ"));
    assert_eq!(parsed.playlist_id, None);
}

#[test]
fn test_parse_youtube_url_short_with_playlist() {
    let parsed = parse_youtube_url("https://youtu.be/dQw4w9WgXcQ?list=PLxyz789");
    assert_eq!(parsed.video_id.as_deref(), Some("dQw4w9WgXcQ"));
    assert_eq!(parsed.playlist_id.as_deref(), Some("PLxyz789"));
}

#[test]
fn test_parse_youtube_url_music() {
    let parsed = parse_youtube_url("https://music.youtube.com/watch?v=abc123&list=OLAK5uy_test");
    assert_eq!(parsed.video_id.as_deref(), Some("abc123"));
    assert_eq!(parsed.playlist_id.as_deref(), Some("OLAK5uy_test"));
}

#[test]
fn test_extract_query_param() {
    assert_eq!(
        extract_query_param("https://example.com?foo=bar&baz=qux", "foo"),
        Some("bar".to_string())
    );
    assert_eq!(
        extract_query_param("https://example.com?foo=bar&baz=qux", "baz"),
        Some("qux".to_string())
    );
    assert_eq!(
        extract_query_param("https://example.com?foo=bar", "missing"),
        None
    );
    assert_eq!(extract_query_param("https://example.com", "foo"), None);
}

#[test]
fn test_parse_yt_initial_data() {
    let html = r#"<script>var ytInitialData = {"key": "value", "nested": {"a": 1}};</script>"#;
    let data = parse_yt_initial_data(html).expect("should parse");
    assert_eq!(data.get("key").and_then(|v| v.as_str()), Some("value"));
}

#[test]
fn test_parse_yt_initial_data_missing() {
    let html = r#"<script>var ytOtherData = {};</script>"#;
    assert!(parse_yt_initial_data(html).is_none());
}

#[test]
fn test_extract_playlist_videos() {
    let yt_data = serde_json::json!({
        "contents": {
            "twoColumnBrowseResultsRenderer": {
                "tabs": [{
                    "tabRenderer": {
                        "content": {
                            "sectionListRenderer": {
                                "contents": [{
                                    "itemSectionRenderer": {
                                        "contents": [{
                                            "playlistVideoListRenderer": {
                                                "contents": [
                                                    {
                                                        "playlistVideoRenderer": {
                                                            "videoId": "abc123",
                                                            "title": {"runs": [{"text": "Track One"}]},
                                                            "index": {"simpleText": "1"},
                                                            "lengthSeconds": "240"
                                                        }
                                                    },
                                                    {
                                                        "playlistVideoRenderer": {
                                                            "videoId": "def456",
                                                            "title": {"runs": [{"text": "Track Two"}]},
                                                            "index": {"simpleText": "2"},
                                                            "lengthSeconds": "180"
                                                        }
                                                    }
                                                ]
                                            }
                                        }]
                                    }
                                }]
                            }
                        }
                    }
                }]
            }
        }
    });

    let videos = extract_playlist_videos(&yt_data);
    assert_eq!(videos.len(), 2);
    assert_eq!(videos[0].video_id, "abc123");
    assert_eq!(videos[0].title, "Track One");
    assert_eq!(videos[0].position, 1);
    assert_eq!(videos[0].duration_ms, Some(240_000));
    assert_eq!(videos[1].video_id, "def456");
    assert_eq!(videos[1].title, "Track Two");
    assert_eq!(videos[1].position, 2);
    assert_eq!(videos[1].duration_ms, Some(180_000));
}

#[test]
fn test_extract_playlist_videos_empty() {
    let yt_data = serde_json::json!({"contents": {}});
    let videos = extract_playlist_videos(&yt_data);
    assert!(videos.is_empty());
}

// =========================================================================
// Bandcamp data-client-items (label page overflow)
// =========================================================================

use super::bandcamp::{extract_data_client_items, parse_client_items, parse_index_page_cells};

#[test]
fn test_extract_data_client_items_basic() {
    let html = r#"<ol class="music-grid" data-client-items="[{&quot;type&quot;:&quot;album&quot;,&quot;title&quot;:&quot;Test&quot;}]"></ol>"#;
    let json = extract_data_client_items(html).expect("should extract JSON");
    assert_eq!(json, r#"[{"type":"album","title":"Test"}]"#);
}

#[test]
fn test_extract_data_client_items_missing() {
    let html = r#"<ol class="music-grid"></ol>"#;
    assert!(extract_data_client_items(html).is_none());
}

#[test]
fn test_extract_data_client_items_empty() {
    let html = r#"<ol data-client-items=""></ol>"#;
    assert!(extract_data_client_items(html).is_none());
}

#[test]
fn test_parse_client_items_albums_and_tracks() {
    let json = r#"[
        {"type":"album","page_url":"/album/first","title":"First Album","artist":"Artist A","art_id":12345},
        {"type":"track","page_url":"/track/single","title":"A Single","artist":null,"art_id":67890},
        {"type":"merch","page_url":"/merch/tshirt","title":"T-Shirt","artist":null,"art_id":null}
    ]"#;
    let page_name = Some("Label Name".to_string());
    let releases = parse_client_items(json, "https://label.bandcamp.com", &page_name);

    assert_eq!(releases.len(), 2);

    assert_eq!(releases[0].url, "https://label.bandcamp.com/album/first");
    assert_eq!(releases[0].title.as_deref(), Some("First Album"));
    assert_eq!(releases[0].artist.as_deref(), Some("Artist A"));
    assert_eq!(
        releases[0].artwork_url.as_deref(),
        Some("https://f4.bcbits.com/img/a12345_16.jpg")
    );

    assert_eq!(releases[1].url, "https://label.bandcamp.com/track/single");
    assert_eq!(releases[1].title.as_deref(), Some("A Single"));
    // No artist on item → falls back to page_name
    assert_eq!(releases[1].artist.as_deref(), Some("Label Name"));
    assert_eq!(
        releases[1].artwork_url.as_deref(),
        Some("https://f4.bcbits.com/img/a67890_16.jpg")
    );
}

#[test]
fn test_parse_client_items_deduplication() {
    // Simulate HTML-parsed releases
    let json = r#"[
        {"type":"album","page_url":"/album/existing","title":"Already There","artist":null,"art_id":111},
        {"type":"album","page_url":"/album/new-one","title":"New Release","artist":null,"art_id":222}
    ]"#;
    let page_name = Some("Artist".to_string());
    let overflow = parse_client_items(json, "https://artist.bandcamp.com", &page_name);

    // Simulate that /album/existing was already found via HTML grid
    let existing_urls: std::collections::HashSet<&str> =
        ["https://artist.bandcamp.com/album/existing"]
            .iter()
            .copied()
            .collect();

    let new_releases: Vec<_> = overflow
        .into_iter()
        .filter(|r| !existing_urls.contains(r.url.as_str()))
        .collect();

    assert_eq!(new_releases.len(), 1);
    assert_eq!(
        new_releases[0].url,
        "https://artist.bandcamp.com/album/new-one"
    );
}

#[test]
fn test_parse_client_items_no_art_id() {
    let json = r#"[{"type":"album","page_url":"/album/no-art","title":"No Art","artist":null}]"#;
    let releases = parse_client_items(json, "https://x.bandcamp.com", &None);
    assert_eq!(releases.len(), 1);
    assert!(releases[0].artwork_url.is_none());
    assert!(releases[0].artist.is_none());
}

// =========================================================================
// Bandcamp index page layout
// =========================================================================

#[test]
fn test_parse_index_page_cells_basic() {
    let html = r#"
        <div class="indexpage_list_cell">
            <a href="/album/cool-ep">
                <img src="https://f4.bcbits.com/img/a123_2.jpg" />
            </a>
            <a href="/album/cool-ep">Cool EP</a>
        </div>
    "#;
    let page_name = Some("Test Label".to_string());
    let releases = parse_index_page_cells(html, "https://test.bandcamp.com", &page_name);

    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].url, "https://test.bandcamp.com/album/cool-ep");
    assert_eq!(releases[0].title.as_deref(), Some("Cool EP"));
    assert_eq!(releases[0].artist.as_deref(), Some("Test Label"));
    assert_eq!(
        releases[0].artwork_url.as_deref(),
        Some("https://f4.bcbits.com/img/a123_2.jpg")
    );
}

#[test]
fn test_parse_index_page_cells_strips_artist_prefix() {
    let html = r#"
        <div class="indexpage_list_cell">
            <a href="/album/my-album"><img src="https://f4.bcbits.com/img/a1_2.jpg" /></a>
            <a href="/album/my-album">Some Artist - My Album</a>
        </div>
    "#;
    let page_name = Some("Some Artist".to_string());
    let releases = parse_index_page_cells(html, "https://x.bandcamp.com", &page_name);

    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].title.as_deref(), Some("My Album"));
}

#[test]
fn test_parse_index_page_cells_filters_non_release() {
    let html = r#"
        <div class="indexpage_list_cell">
            <a href="/merch/tshirt"><img src="https://f4.bcbits.com/img/a1_2.jpg" /></a>
            <a href="/merch/tshirt">Cool T-Shirt</a>
        </div>
        <div class="indexpage_list_cell">
            <a href="/album/real-album"><img src="https://f4.bcbits.com/img/a2_2.jpg" /></a>
            <a href="/album/real-album">Real Album</a>
        </div>
    "#;
    let releases = parse_index_page_cells(html, "https://x.bandcamp.com", &None);

    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].url, "https://x.bandcamp.com/album/real-album");
}

#[test]
fn test_parse_index_page_cells_multiple() {
    let html = r#"
        <div class="indexpage_list_cell">
            <a href="/album/first"><img src="https://f4.bcbits.com/img/a1_2.jpg" /></a>
            <a href="/album/first">First</a>
        </div>
        <div class="indexpage_list_cell">
            <a href="/album/second"><img src="https://f4.bcbits.com/img/a2_2.jpg" /></a>
            <a href="/album/second">Second</a>
        </div>
        <div class="indexpage_list_cell">
            <a href="/track/third"><img src="https://f4.bcbits.com/img/a3_2.jpg" /></a>
            <a href="/track/third">Third</a>
        </div>
    "#;
    let releases = parse_index_page_cells(html, "https://x.bandcamp.com", &None);

    assert_eq!(releases.len(), 3);
    assert_eq!(releases[0].url, "https://x.bandcamp.com/album/first");
    assert_eq!(releases[1].url, "https://x.bandcamp.com/album/second");
    assert_eq!(releases[2].url, "https://x.bandcamp.com/track/third");
}

#[test]
fn test_parse_index_page_cells_empty() {
    let html = r#"<div class="main-content"><p>No releases here</p></div>"#;
    let releases = parse_index_page_cells(html, "https://x.bandcamp.com", &None);
    assert!(releases.is_empty());
}
