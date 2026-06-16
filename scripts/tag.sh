#!/bin/bash
#
# Release tagging script for Crate
#
# Usage:
#   ./scripts/tag.sh minor staging        # Start staging prerelease
#   ./scripts/tag.sh prerelease           # Iterate current prerelease
#   ./scripts/tag.sh stage                # Promote staging to stable
#   ./scripts/tag.sh --test stage         # Test build before pushing
#   ./scripts/tag.sh --dry-run minor      # Preview without executing
#   ./scripts/tag.sh --delete v0.2.0      # Delete local and remote tag

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Flags
DRY_RUN=false
TEST_BUILD=false
DELETE_TAG=""

# Parse flags
while [[ "$1" == --* ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --test)
            TEST_BUILD=true
            shift
            ;;
        --delete)
            DELETE_TAG="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Unknown flag: $1${NC}"
            exit 1
            ;;
    esac
done

# Handle delete mode
if [[ -n "$DELETE_TAG" ]]; then
    echo -e "${BLUE}Deleting tag: $DELETE_TAG${NC}"

    if $DRY_RUN; then
        echo -e "${YELLOW}[DRY RUN] Would delete local tag: $DELETE_TAG${NC}"
        echo -e "${YELLOW}[DRY RUN] Would delete remote tag: $DELETE_TAG${NC}"
    else
        # Delete local tag
        if git tag -d "$DELETE_TAG" 2>/dev/null; then
            echo -e "${GREEN}Deleted local tag: $DELETE_TAG${NC}"
        else
            echo -e "${YELLOW}Local tag not found: $DELETE_TAG${NC}"
        fi

        # Delete remote tag
        if git push origin ":refs/tags/$DELETE_TAG" 2>/dev/null; then
            echo -e "${GREEN}Deleted remote tag: $DELETE_TAG${NC}"
        else
            echo -e "${YELLOW}Remote tag not found or already deleted: $DELETE_TAG${NC}"
        fi
    fi
    exit 0
fi

# Validate arguments
if [[ -z "$1" ]]; then
    echo -e "${RED}Error: No bump type specified${NC}"
    echo ""
    echo "Usage:"
    echo "  ./scripts/tag.sh <major|minor|patch> staging   # Version bump with staging prerelease"
    echo "  ./scripts/tag.sh prerelease                     # Increment prerelease"
    echo "  ./scripts/tag.sh stage                          # Promote staging to stable"
    echo ""
    echo "Flags:"
    echo "  --dry-run              Preview actions without executing"
    echo "  --test                 Run build test before pushing"
    echo "  --delete <tag>         Delete a tag from local and remote"
    exit 1
fi

BUMP_TYPE="$1"
CHANNEL="$2"

# Validate channel
if [[ -n "$CHANNEL" && "$CHANNEL" != "staging" ]]; then
    echo -e "${RED}Error: Invalid channel '$CHANNEL'. Only 'staging' is supported.${NC}"
    exit 1
fi

# Change to root directory
cd "$ROOT_DIR"

# Get current version
OLD_VERSION=$(node -p "require('./package.json').version")
echo -e "${BLUE}Current version: $OLD_VERSION${NC}"

# Determine new version using the canonical version.js script
if [[ -n "$CHANNEL" ]]; then
    NEW_VERSION=$(node scripts/version.js --print "$BUMP_TYPE" "$CHANNEL")
else
    NEW_VERSION=$(node scripts/version.js --print "$BUMP_TYPE")
fi

# Determine if this is going to stable
IS_STABLE=true
if [[ "$NEW_VERSION" =~ -staging\. ]]; then
    IS_STABLE=false
fi

echo -e "${GREEN}New version: $NEW_VERSION${NC}"
echo ""

if $DRY_RUN; then
    echo -e "${YELLOW}=== DRY RUN ===${NC}"
    echo ""
fi

# Step 1: Bump version
echo -e "${BLUE}Step 1: Bumping version...${NC}"
if $DRY_RUN; then
    echo -e "${YELLOW}[DRY RUN] Would run: yarn bump $BUMP_TYPE $CHANNEL${NC}"
else
    if [[ -n "$CHANNEL" ]]; then
        yarn bump "$BUMP_TYPE" "$CHANNEL"
    else
        yarn bump "$BUMP_TYPE"
    fi
fi
echo ""

# Step 1b: Update Cargo.lock to reflect new version
echo -e "${BLUE}Step 1b: Updating Cargo.lock...${NC}"
if $DRY_RUN; then
    echo -e "${YELLOW}[DRY RUN] Would run: cargo generate-lockfile --manifest-path src-tauri/Cargo.toml${NC}"
else
    cargo generate-lockfile --manifest-path src-tauri/Cargo.toml
fi
echo ""

# Step 2: Prepare/graduate changelog
echo -e "${BLUE}Step 2: Updating changelog...${NC}"
if [[ "$BUMP_TYPE" == "prerelease" ]]; then
    # Prerelease increment — no new changelog entry needed
    echo -e "${YELLOW}Skipping changelog (prerelease increment)${NC}"
elif $IS_STABLE && [[ "$BUMP_TYPE" == "stage" ]]; then
    # Graduating to stable - consolidate prerelease entries
    if $DRY_RUN; then
        echo -e "${YELLOW}[DRY RUN] Would run: yarn changelog:graduate $NEW_VERSION${NC}"
    else
        yarn changelog:graduate "$NEW_VERSION"
    fi
else
    # Regular prepare
    if $DRY_RUN; then
        echo -e "${YELLOW}[DRY RUN] Would run: yarn changelog:prepare $NEW_VERSION${NC}"
    else
        yarn changelog:prepare "$NEW_VERSION"
    fi
fi
echo ""

# Step 3: Commit changes
echo -e "${BLUE}Step 3: Committing changes...${NC}"
if $DRY_RUN; then
    echo -e "${YELLOW}[DRY RUN] Would stage: package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/tauri.staging.conf.json CHANGELOG.md${NC}"
    echo -e "${YELLOW}[DRY RUN] Would run: git commit -m \"chore: release v$NEW_VERSION\"${NC}"
else
    git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock \
        src-tauri/tauri.conf.json src-tauri/tauri.dev.conf.json src-tauri/tauri.staging.conf.json \
        CHANGELOG.md
    git commit -m "chore: release \`v$NEW_VERSION\`"
fi
echo ""

# Step 4: Create tag
echo -e "${BLUE}Step 4: Creating tag...${NC}"
if $DRY_RUN; then
    echo -e "${YELLOW}[DRY RUN] Would run: git tag v$NEW_VERSION${NC}"
else
    git tag -m "Release \`v$NEW_VERSION\`" "v$NEW_VERSION"
fi
echo ""

# Step 5: Optional build test
if $TEST_BUILD; then
    echo -e "${BLUE}Step 5: Testing build...${NC}"
    if $DRY_RUN; then
        echo -e "${YELLOW}[DRY RUN] Would run: yarn tauri build --features desktop${NC}"
    else
        yarn tauri build --features desktop
    fi
    echo ""
fi

# Step 6: Push to remote
echo -e "${BLUE}Step 6: Pushing to remote...${NC}"
if $DRY_RUN; then
    echo -e "${YELLOW}[DRY RUN] Would run: git push origin HEAD${NC}"
    echo -e "${YELLOW}[DRY RUN] Would run: git push origin v$NEW_VERSION${NC}"
else
    git push origin HEAD
    git push origin "v$NEW_VERSION"
fi
echo ""

# Done
if $DRY_RUN; then
    echo -e "${YELLOW}=== DRY RUN COMPLETE ===${NC}"
    echo -e "${YELLOW}No changes were made. Run without --dry-run to execute.${NC}"
else
    echo -e "${GREEN}=== Released v$NEW_VERSION ===${NC}"
    echo -e "${GREEN}GitHub Actions will now build and create a draft release.${NC}"
fi
