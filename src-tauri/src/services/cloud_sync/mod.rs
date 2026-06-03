//! Cross-device cloud sync.
//!
//! Phase 0 landed the local foundations: a hybrid logical clock ([`hlc`]), the
//! per-mutation change-tracking hooks ([`pipeline::dirty`]), bucket identity
//! ([`pipeline::buckets`]), and library-root path resolution ([`resolution`]).
//!
//! Phase 1 adds the local sync core: per-bucket JSONL serialize/parse
//! ([`pipeline::rows`]), the HLC + tombstone merge engine ([`pipeline::merge`]),
//! local manifest computation ([`pipeline::manifest`]), and the vendor-agnostic
//! [`backend`] trait surface with an in-memory mock. Still no network I/O — Firebase
//! arrives in Phase 2.
//!
//! The mutation hooks run unconditionally (even when sync is disabled) so the dirty
//! queue and HLC stamps are ready the moment a user opts in — this is what lets a
//! "sync off → mutate → sync on" sequence flush every change.
#![allow(dead_code)]

pub mod auth;
pub mod backend;
pub mod config;
pub mod hlc;
pub mod pipeline;
pub mod resolution;
pub mod runtime;

#[cfg(test)]
mod tests;

/// Percent-encode a string for use as a single URL path segment or query value.
/// Encodes everything outside the RFC 3986 unreserved set (so `/` → `%2F`). Shared
/// by the auth flow and the Firebase REST layer to avoid pulling in the `url` crate.
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Settings keys that sync across devices (LWW per key, stamped in `sync_state`
/// under `setting_hlc:<key>`). Everything else stays device-local: `audio_device`,
/// `has_completed_onboarding`, `has_completed_wizard`, `ignored_device_ids`, all
/// backup bookkeeping, and all cloud-sync state itself.
pub const SYNCED_SETTING_KEYS: &[&str] = &[
    "theme",
    "accent_color",
    "language",
    "date_format",
    "key_notation_format",
    "auto_analyze_on_import",
];

/// Whether a settings key participates in cloud sync.
pub fn is_synced_setting(key: &str) -> bool {
    SYNCED_SETTING_KEYS.contains(&key)
}
