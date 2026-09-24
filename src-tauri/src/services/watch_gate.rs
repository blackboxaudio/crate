//! Shared rate-limit gating for the periodic scrape loops (follow watch, collection
//! refresh): a per-target re-scan cooldown for automatic sweeps, plus a failure backoff
//! that grows with the consecutive-failure streak and is honored even for forced manual
//! checks — a user can't hammer a platform that's already throttling us.

use chrono::{DateTime, Utc};

use crate::models::FollowHealth;

/// Base backoff after a failure, doubled per consecutive failure (capped). Rate-limit
/// responses (HTTP 429) start longer than transient errors.
pub const BACKOFF_BASE_ERROR_SECS: i64 = 5 * 60;
pub const BACKOFF_BASE_RATE_LIMITED_SECS: i64 = 15 * 60;
pub const BACKOFF_MAX_SECS: i64 = 6 * 60 * 60;

/// The local check-state fields a gate decision needs, read from the per-device state
/// table (`followed_source_state` / `collection_account_state`).
pub struct CheckGate {
    pub last_checked_at: Option<String>,
    pub health: String,
    pub last_error: Option<String>,
    pub consecutive_failures: i64,
}

/// Classify a scan error as a rate-limit (so the UI shows it and backoff runs longer) or
/// a generic failure. Bandcamp/SoundCloud/Discogs all surface 429s with a "rate limit"
/// / "429" marker in the message (see the metadata fetchers).
pub fn health_for_error(msg: &str) -> FollowHealth {
    let m = msg.to_lowercase();
    if m.contains("rate limit") || m.contains("429") || m.contains("too many requests") {
        FollowHealth::RateLimited
    } else {
        FollowHealth::Error
    }
}

/// Seconds elapsed since an RFC3339 timestamp, or `None` if it can't be parsed or is in
/// the future (a clock jump — treat as "unknown", i.e. don't skip).
pub fn seconds_since(now: DateTime<Utc>, ts: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|t| {
            now.signed_duration_since(t.with_timezone(&Utc))
                .num_seconds()
        })
        .filter(|&s| s >= 0)
}

/// The backoff window (seconds) implied by the last check's health + failure streak.
/// Zero when the last check succeeded (`consecutive_failures == 0`).
pub fn backoff_window_secs(health: &str, consecutive_failures: i64) -> i64 {
    if consecutive_failures < 1 {
        return 0;
    }
    let base = if health == "rate_limited" {
        BACKOFF_BASE_RATE_LIMITED_SECS
    } else {
        BACKOFF_BASE_ERROR_SECS
    };
    let shift = (consecutive_failures - 1).min(10) as u32;
    base.saturating_mul(1i64 << shift).min(BACKOFF_MAX_SECS)
}

/// Whether to skip a target's network scan right now. A failure backoff applies even to
/// a forced manual check; the plain re-scan cooldown (`rescan_cooldown_secs`, per loop)
/// applies only to automatic sweeps (`force == false`). Returns `false` when there's no
/// prior check to gate against.
pub fn should_skip_scan(
    now: DateTime<Utc>,
    gate: &CheckGate,
    force: bool,
    rescan_cooldown_secs: i64,
) -> bool {
    let elapsed = match gate
        .last_checked_at
        .as_deref()
        .and_then(|ts| seconds_since(now, ts))
    {
        Some(e) => e,
        None => return false,
    };
    let backoff = backoff_window_secs(&gate.health, gate.consecutive_failures);
    if backoff > 0 && elapsed < backoff {
        return true;
    }
    !force && elapsed < rescan_cooldown_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The follow loop's cooldown — the value these gate tests were written against.
    const COOLDOWN: i64 = 30 * 60;

    fn at(ts: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(ts)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn gate(last_checked_at: Option<&str>, health: &str, failures: i64) -> CheckGate {
        CheckGate {
            last_checked_at: last_checked_at.map(|s| s.to_string()),
            health: health.to_string(),
            last_error: None,
            consecutive_failures: failures,
        }
    }

    #[test]
    fn never_checked_source_is_never_skipped() {
        let now = at("2026-07-05T12:00:00Z");
        assert!(!should_skip_scan(
            now,
            &gate(None, "unknown", 0),
            false,
            COOLDOWN
        ));
        assert!(!should_skip_scan(
            now,
            &gate(None, "unknown", 0),
            true,
            COOLDOWN
        ));
    }

    #[test]
    fn cooldown_skips_recent_ok_check_only_on_automatic_sweeps() {
        let now = at("2026-07-05T12:00:00Z");
        // Checked 10 min ago, healthy: within the 30-min cooldown.
        let recent = gate(Some("2026-07-05T11:50:00Z"), "ok", 0);
        assert!(should_skip_scan(now, &recent, false, COOLDOWN)); // automatic sweep: skip
        assert!(!should_skip_scan(now, &recent, true, COOLDOWN)); // forced manual: scan
    }

    #[test]
    fn cooldown_expires_after_the_window() {
        let now = at("2026-07-05T12:00:00Z");
        // Checked 31 min ago, healthy: past the cooldown.
        let old = gate(Some("2026-07-05T11:29:00Z"), "ok", 0);
        assert!(!should_skip_scan(now, &old, false, COOLDOWN));
    }

    #[test]
    fn longer_cooldowns_gate_longer() {
        let now = at("2026-07-05T12:00:00Z");
        // Checked 2h ago: past follow's 30-min cooldown, within collection's 6h one.
        let g = gate(Some("2026-07-05T10:00:00Z"), "ok", 0);
        assert!(!should_skip_scan(now, &g, false, COOLDOWN));
        assert!(should_skip_scan(now, &g, false, 6 * 3600));
    }

    #[test]
    fn rate_limit_backoff_is_honored_even_when_forced() {
        let now = at("2026-07-05T12:00:00Z");
        // Rate-limited 5 min ago, first failure → 15-min backoff window.
        let limited = gate(Some("2026-07-05T11:55:00Z"), "rate_limited", 1);
        assert!(should_skip_scan(now, &limited, false, COOLDOWN));
        assert!(should_skip_scan(now, &limited, true, COOLDOWN)); // forced still backs off
    }

    #[test]
    fn backoff_grows_with_consecutive_failures_and_caps() {
        // error base 5m: failure #1 → 5m, #2 → 10m, #3 → 20m.
        assert_eq!(backoff_window_secs("error", 1), 5 * 60);
        assert_eq!(backoff_window_secs("error", 2), 10 * 60);
        assert_eq!(backoff_window_secs("error", 3), 20 * 60);
        // rate_limited base 15m: failure #1 → 15m.
        assert_eq!(backoff_window_secs("rate_limited", 1), 15 * 60);
        // A large streak saturates at the cap, never overflows.
        assert_eq!(backoff_window_secs("rate_limited", 100), BACKOFF_MAX_SECS);
        // A successful last check (no streak) implies no backoff.
        assert_eq!(backoff_window_secs("ok", 0), 0);
    }

    #[test]
    fn future_timestamp_from_clock_jump_does_not_skip() {
        let now = at("2026-07-05T12:00:00Z");
        let future = gate(Some("2026-07-05T12:30:00Z"), "ok", 0);
        assert!(!should_skip_scan(now, &future, false, COOLDOWN));
    }

    #[test]
    fn error_messages_classify_rate_limits() {
        assert_eq!(
            health_for_error("Bandcamp rate limit exceeded (429)"),
            FollowHealth::RateLimited
        );
        assert_eq!(
            health_for_error("Discogs API returned status 429 Too Many Requests"),
            FollowHealth::RateLimited
        );
        assert_eq!(
            health_for_error("Failed to fetch page: connection reset"),
            FollowHealth::Error
        );
    }
}
