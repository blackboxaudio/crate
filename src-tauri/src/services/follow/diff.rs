//! Pure, unit-testable helpers for the follow watch loop.

use std::collections::HashMap;

use crate::models::ScannedRelease;

/// Releases on the page that are NEW for this source: those whose normalized URL is
/// not yet recorded in the source's seen set (baseline ∪ surfaced ∪ dismissed). The
/// caller decides create-vs-attach-provenance per URL against live Discovery — a URL
/// new to this source may already be in Discovery (manually added or surfaced by
/// another follow this same sweep), in which case it is deduped, not duplicated.
pub fn compute_new_urls(
    found: &[ScannedRelease],
    seen: &HashMap<String, String>,
) -> Vec<ScannedRelease> {
    found
        .iter()
        .filter(|r| !seen.contains_key(&r.url))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(url: &str) -> ScannedRelease {
        ScannedRelease {
            url: url.to_string(),
            artist: None,
            title: None,
            artwork_url: None,
            release_date: None,
            already_exists: false,
        }
    }

    #[test]
    fn new_urls_excludes_seen_regardless_of_status() {
        let found = vec![rel("a"), rel("b"), rel("c")];
        let mut seen = HashMap::new();
        seen.insert("a".to_string(), "baseline".to_string());
        seen.insert("c".to_string(), "dismissed".to_string());
        let out = compute_new_urls(&found, &seen);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "b");
    }

    #[test]
    fn new_urls_all_new_when_seen_empty() {
        let found = vec![rel("a"), rel("b")];
        let seen = HashMap::new();
        assert_eq!(compute_new_urls(&found, &seen).len(), 2);
    }

    #[test]
    fn new_urls_empty_when_all_seen() {
        let found = vec![rel("a"), rel("b")];
        let mut seen = HashMap::new();
        seen.insert("a".to_string(), "surfaced".to_string());
        seen.insert("b".to_string(), "baseline".to_string());
        assert!(compute_new_urls(&found, &seen).is_empty());
    }
}
