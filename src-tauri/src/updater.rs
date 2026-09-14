//! Channel-aware acceptance rule for the auto-updater.
//!
//! Each channel polls its own manifest (`staging/latest.json` vs `production/latest.json`),
//! but the plugin's default comparator is plain semver, under which a stable `0.3.0` beats
//! `0.3.0-staging.47` and `0.3.0-staging.47` beats a stable `0.2.9`. Both channels are signed
//! with the same key, so a build that ever read the other channel's manifest would silently
//! cross-grade. This module makes the channel part of the comparison so that can't happen,
//! whichever endpoint a binary was built with.

use semver::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseChannel {
    Production,
    Staging,
    /// Any other prerelease tag (e.g. `-dev`); never matched by a published manifest.
    Other,
}

impl std::fmt::Display for ReleaseChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Production => "production",
            Self::Staging => "staging",
            Self::Other => "other",
        };
        f.write_str(name)
    }
}

fn channel_of(version: &Version) -> ReleaseChannel {
    if version.pre.is_empty() {
        return ReleaseChannel::Production;
    }
    match version.pre.as_str().split('.').next() {
        Some("staging") => ReleaseChannel::Staging,
        _ => ReleaseChannel::Other,
    }
}

/// Whether `remote` should be offered to a build running `current`: it must be newer *and*
/// on the same channel. A channel mismatch means the binary is reading the wrong manifest, so
/// it is logged rather than silently reported as "up to date".
pub fn accepts_release(current: &Version, remote: &Version) -> bool {
    let (current_channel, remote_channel) = (channel_of(current), channel_of(remote));
    if current_channel != remote_channel {
        log::warn!(
            "updater: ignoring {remote} ({remote_channel} channel) — this build is {current} ({current_channel} channel); \
             the configured update endpoint does not match this build's channel"
        );
        return false;
    }
    remote > current
}

#[cfg(feature = "desktop")]
pub fn version_comparator(current: Version, release: tauri_plugin_updater::RemoteRelease) -> bool {
    accepts_release(&current, &release.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepts(current: &str, remote: &str) -> bool {
        accepts_release(&Version::parse(current).unwrap(), &Version::parse(remote).unwrap())
    }

    #[test]
    fn staging_accepts_newer_staging() {
        assert!(accepts("0.3.0-staging.45", "0.3.0-staging.47"));
        assert!(accepts("0.3.0-staging.47", "0.3.1-staging.1"));
    }

    #[test]
    fn staging_rejects_same_or_older_staging() {
        assert!(!accepts("0.3.0-staging.47", "0.3.0-staging.47"));
        assert!(!accepts("0.3.0-staging.47", "0.3.0-staging.45"));
    }

    #[test]
    fn staging_never_takes_a_stable_release() {
        assert!(!accepts("0.3.0-staging.47", "0.2.9"));
        // Plain semver would accept this one: 0.3.0 > 0.3.0-staging.47.
        assert!(!accepts("0.3.0-staging.47", "0.3.0"));
        assert!(!accepts("0.3.0-staging.47", "0.4.0"));
    }

    #[test]
    fn production_accepts_only_newer_stable() {
        assert!(accepts("0.2.9", "0.3.0"));
        assert!(!accepts("0.3.0", "0.2.9"));
        assert!(!accepts("0.3.0", "0.3.0"));
    }

    #[test]
    fn production_never_takes_a_prerelease() {
        // Plain semver would accept this one: 0.3.0-staging.47 > 0.2.9.
        assert!(!accepts("0.2.9", "0.3.0-staging.47"));
        assert!(!accepts("0.2.9", "0.3.0-dev"));
    }

    #[test]
    fn dev_builds_never_update() {
        assert!(!accepts("0.3.0-dev", "0.3.0"));
        assert!(!accepts("0.3.0-dev", "0.3.0-staging.47"));
        assert!(!accepts("0.3.0-dev", "0.4.0-dev"));
    }
}
