//! Cross-device cloud sync.
//!
//! Phase 0 lands the local foundations: a hybrid logical clock ([`hlc`]), the
//! per-mutation change-tracking hooks ([`pipeline::dirty`]), bucket identity
//! ([`pipeline::buckets`]), and library-root path resolution ([`resolution`]).
//! Later phases add serialization/merge, the backend traits, and Firebase.
//!
//! Nothing here performs any network I/O. The mutation hooks run unconditionally
//! (even when sync is disabled) so the dirty queue and HLC stamps are ready the
//! moment a user opts in — this is what lets a "sync off → mutate → sync on"
//! sequence flush every change.
#![allow(dead_code)]

pub mod hlc;
pub mod pipeline;
pub mod resolution;

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
