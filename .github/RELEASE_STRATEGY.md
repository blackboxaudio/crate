# Release Strategy

## Overview

Crate uses a two-channel release pipeline: **staging** (prerelease) and **production** (stable). All releases are tagged on the `develop` branch and built automatically by CI/CD. There is no alpha/beta distinction — staging is the single prerelease channel.

## Branch Model

| Branch | Purpose |
|--------|---------|
| `develop` | Main integration branch. All PRs target here. Releases are tagged here. |
| `{issue}-{description}` | Feature/fix branches. Named after the GitHub issue number. |

There is no long-lived `main` or `release/*` branch. `develop` is the source of truth.

## Release Channels

| Channel | Tag format | Example | Config | Devtools |
|---------|-----------|---------|--------|----------|
| **Staging** | `v0.X.Y-staging.N` | `v0.2.0-staging.3` | `tauri.staging.conf.json` | Enabled |
| **Production** | `v0.X.Y` | `v0.2.0` | `tauri.prod.conf.json` | Disabled |

Each channel has its own auto-updater endpoint (`staging/latest.json` and `production/latest.json` in GCS), so staging and production users receive updates independently.

## Version Strategy

The canonical version lives in `package.json`. The `yarn bump` command (`scripts/version.js`) synchronizes it across all files:

| File | Version format | Example |
|------|---------------|---------|
| `package.json` | Full semver | `0.2.0-staging.3` |
| `src-tauri/Cargo.toml` | Full semver | `0.2.0-staging.3` |
| `src-tauri/tauri.conf.json` | Base only (no prerelease) | `0.2.0` |
| `src-tauri/tauri.staging.conf.json` | MSI-compatible numeric prerelease | `0.2.0-3` |

The Tauri configs use numeric-only versions because Windows MSI bundling requires it. The staging config overrides the version only during prereleases; for stable releases it inherits from the base config.

## Release Workflow

### Quick Reference

| Action | Command |
|--------|---------|
| Start staging prerelease | `./scripts/tag.sh minor staging` |
| Iterate staging | `./scripts/tag.sh prerelease` |
| Promote to stable | `./scripts/tag.sh stage` |
| Standard stable release | `./scripts/tag.sh minor` |
| Preview without executing | `./scripts/tag.sh --dry-run <args>` |
| Test build before pushing | `./scripts/tag.sh --test <args>` |
| Delete a tag | `./scripts/tag.sh --delete v0.2.0-staging.1` |

### Flow 1: Start a Staging Prerelease

```
0.1.0 → 0.2.0-staging.1
```

```bash
./scripts/tag.sh minor staging
```

This bumps the minor version, enters the staging channel, prepares a changelog entry, commits, tags, and pushes.

### Flow 2: Iterate on Staging

```
0.2.0-staging.1 → 0.2.0-staging.2 → 0.2.0-staging.3
```

```bash
./scripts/tag.sh prerelease
```

Increments the prerelease number. Changelog is skipped for prerelease increments (changes accumulate under the initial staging entry).

### Flow 3: Promote to Stable

```
0.2.0-staging.3 → 0.2.0
```

```bash
./scripts/tag.sh stage
```

Strips the prerelease suffix, graduates the changelog (consolidates all staging entries into a single stable entry), commits, tags, and pushes.

### What Happens Under the Hood

The `tag.sh` script runs these steps in order:

1. **Bump version** — `yarn bump <type> [channel]` updates all version files
2. **Update changelog** — `yarn changelog:prepare` or `yarn changelog:graduate` (skipped for prerelease increments)
3. **Commit** — Stages the modified files and commits with `chore: release v<version>`
4. **Tag** — Creates an annotated git tag `v<version>`
5. **Build test** (optional) — Runs `yarn tauri build` if `--test` was passed
6. **Push** — Pushes the commit and tag to `origin`

## CI/CD Pipeline

Pushing a `v*` tag triggers the `cd.release.yml` workflow:

1. **Validate** — Checks semver format, verifies changelog entry exists, determines channel (staging or production) and MSI-compatible bundle version
2. **Build** — Parallel builds on macOS (universal binary) and Windows (MSI + NSIS). Uses the appropriate Tauri config based on channel.
3. **Release** — Creates a draft GitHub release with artifacts, then uploads updater artifacts (`*.app.tar.gz`, `*.msi.zip`, and their signatures) to GCS and generates `latest.json` for the auto-updater

The updater artifacts land at `gs://crate-releases/{channel}/{version}/` and the `latest.json` is written to `gs://crate-releases/{channel}/latest.json`.

## Changelog Management

The changelog (`CHANGELOG.md`) follows [Keep a Changelog](https://keepachangelog.com/) format with two commands:

| Command | When to use |
|---------|-------------|
| `yarn changelog:prepare <version>` | Starting a new staging prerelease or a direct stable release. Moves `[Unreleased]` content into a dated version section. |
| `yarn changelog:graduate <version>` | Promoting staging to stable. Consolidates all `X.Y.Z-staging.*` entries into a single `X.Y.Z` entry. |

Prerelease increments (`./scripts/tag.sh prerelease`) skip the changelog entirely — changes continue accumulating under `[Unreleased]` until the next `prepare` or `graduate`.

## Mobile Distribution

iOS (TestFlight / App Store) and Android (signed APK) builds run alongside the desktop build in the same `cd.release.yml` workflow. See [MOBILE_DISTRIBUTION.md](MOBILE_DISTRIBUTION.md) for setup instructions, required GitHub secrets, and the full signing flow.

## Post-Release Checklist

After CI completes:

1. Go to [GitHub Releases](https://github.com/blackboxaudio/crate/releases) and review the draft
2. Verify artifacts are attached (DMG for macOS, MSI/EXE for Windows, APK + sha256 for Android)
3. Review release notes
4. Publish the release
5. Verify download links work
6. Verify auto-updater picks up the new version (check `latest.json` in GCS)
7. Verify the iOS build appeared in TestFlight (App Store Connect → TestFlight)
8. `sha256sum -c` the Android APK from the release, then sideload to verify

## Utilities

| Flag | Description |
|------|-------------|
| `--dry-run` | Preview every step without executing. Useful to verify the version bump and changelog operation before committing. |
| `--test` | Run a full `yarn tauri build` after committing but before pushing. Catches build failures before the tag goes remote. |
| `--delete <tag>` | Delete a tag from both local and remote. Use when a release needs to be re-done (e.g., `./scripts/tag.sh --delete v0.2.0-staging.1`). |
