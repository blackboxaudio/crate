# Release Checklist

## Quick Release (Recommended)

Use the `tag.sh` script for a streamlined release process:

```bash
# Start an alpha prerelease
./scripts/tag.sh minor alpha       # 0.1.0 -> 0.2.0-alpha.1

# Iterate on current prerelease
./scripts/tag.sh prerelease        # 0.2.0-alpha.1 -> 0.2.0-alpha.2

# Promote to beta
./scripts/tag.sh stage             # 0.2.0-alpha.2 -> 0.2.0-beta.1

# Promote to stable (consolidates changelog)
./scripts/tag.sh stage             # 0.2.0-beta.1 -> 0.2.0

# Standard stable release
./scripts/tag.sh minor             # 0.1.0 -> 0.2.0
```

### Options

```bash
--dry-run    # Preview actions without executing
--test       # Run build test before pushing
--delete     # Delete a tag: ./scripts/tag.sh --delete v0.2.0-alpha.1
```

---

## Manual Release Process

### Before Release

- [ ] All features for this release are merged to `develop`
- [ ] CHANGELOG.md has all changes documented in `[Unreleased]` section
- [ ] Version bump type is decided (major, minor, or patch)

### Stable Release

1. [ ] Run version bump: `yarn bump <major|minor|patch>`
2. [ ] Prepare changelog: `yarn changelog:prepare <version>`
3. [ ] Commit: `git commit -am "chore: release vX.Y.Z"`
4. [ ] Create tag: `git tag vX.Y.Z`
5. [ ] Push: `git push origin HEAD && git push origin vX.Y.Z`

### Prerelease (Alpha/Beta)

#### Starting a Prerelease

1. [ ] Run version bump: `yarn bump minor alpha` (or `major`/`patch`)
2. [ ] Prepare changelog: `yarn changelog:prepare 0.2.0-alpha.1`
3. [ ] Commit: `git commit -am "chore: release v0.2.0-alpha.1"`
4. [ ] Tag and push: `git tag v0.2.0-alpha.1 && git push origin HEAD && git push origin v0.2.0-alpha.1`

#### Iterating on Prerelease

1. [ ] Document changes in `[Unreleased]` section
2. [ ] Run: `yarn bump prerelease`
3. [ ] Prepare changelog: `yarn changelog:prepare 0.2.0-alpha.2`
4. [ ] Commit, tag, and push

#### Promoting to Beta

1. [ ] Run: `yarn bump stage` (moves alpha -> beta)
2. [ ] Prepare changelog: `yarn changelog:prepare 0.2.0-beta.1`
3. [ ] Commit, tag, and push

#### Promoting to Stable

1. [ ] Graduate changelog: `yarn changelog:graduate 0.2.0` (consolidates all prereleases)
2. [ ] Run: `yarn bump stage` (moves beta -> stable)
3. [ ] Commit: `git commit -am "chore: release v0.2.0"`
4. [ ] Tag and push: `git tag v0.2.0 && git push origin HEAD && git push origin v0.2.0`

---

## Publish Release

1. [ ] Go to GitHub Releases page
2. [ ] Review the draft release
3. [ ] Verify artifacts are attached (DMG, MSI, EXE)
4. [ ] Review release notes
5. [ ] Click "Publish release"

## After Release

- [ ] Verify download links work
- [ ] Announce release (if applicable)

---

## Deleting a Tag

If you need to remove a tag (e.g., to re-release):

```bash
./scripts/tag.sh --delete v0.2.0-alpha.1
```

This removes the tag from both local and remote.
