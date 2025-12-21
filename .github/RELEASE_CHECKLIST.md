# Release Checklist

## Before Release

- [ ] All features for this release are merged to `develop`
- [ ] CHANGELOG.md has all changes documented in `[Unreleased]` section
- [ ] Version bump type is decided (major, minor, or patch)

## Prepare Release

1. [ ] Run version bump: `yarn bump <major|minor|patch>`
2. [ ] Convert `[Unreleased]` section to `[X.Y.Z] - YYYY-MM-DD` in CHANGELOG.md
3. [ ] Add new empty `[Unreleased]` section at top
4. [ ] Add comparison link at bottom of CHANGELOG.md:
   ```
   [X.Y.Z]: https://github.com/blackboxaudio/crate/compare/vPREV...vX.Y.Z
   ```
5. [ ] Update `[Unreleased]` link to compare against new version
6. [ ] Commit: `git commit -m "chore: prepare release vX.Y.Z"`
7. [ ] Push to develop

## Trigger Release

### Option A: Tag-based release (recommended)

1. [ ] Create the release tag: `git tag vX.Y.Z`
2. [ ] Push the tag: `git push origin vX.Y.Z`
3. [ ] Wait for the Release workflow to complete (triggered automatically)

### Option B: Manual build (for distribution without formal release)

1. [ ] Go to Actions > Release > Run workflow
2. [ ] Enter version number (e.g., `0.2.0`)
3. [ ] Check "prerelease" if this is a beta/RC release
4. [ ] Click "Run workflow"
5. [ ] Wait for builds to complete

## Publish Release

1. [ ] Go to Releases page
2. [ ] Review the draft release
3. [ ] Verify artifacts are attached (DMG, MSI, EXE)
4. [ ] Review release notes
5. [ ] Click "Publish release"

## After Release

- [ ] Verify download links work
- [ ] Announce release (if applicable)
