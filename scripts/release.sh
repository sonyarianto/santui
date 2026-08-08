#!/usr/bin/env bash
# Release automation: bump version everywhere, changelog, tag, push, release.
#
# Usage: ./scripts/release.sh <version>     e.g. ./scripts/release.sh 0.2.40
#
# Steps:
#   1. Pre-flight: clean tree + every version-bearing file contains OLD version
#   2. Bump all version-bearing files (Cargo.toml, npm, website, plugins.json)
#   3. Verify NO leftover OLD version outside CHANGELOG.md/.git
#   4. cargo check sanity, commit "chore: bump version to X"
#   5. git cliff (pre-tag, [unreleased] section), commit
#   6. Tag vX + push (triggers CI)
#   7. git cliff (post-tag, named section), commit, push
#   8. Create GitHub release from the new changelog section
#
# Prerequisites: git-cliff, gh (authed), clean working tree.

set -euo pipefail

NEW="${1:?usage: ./scripts/release.sh <version>}"
if ! [[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "❌ Invalid version '$NEW' — expected x.y.z"
    exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v git-cliff >/dev/null || { echo "❌ git-cliff not installed"; exit 1; }
command -v gh >/dev/null || { echo "❌ gh not installed"; exit 1; }

OLD="$(grep '^version' crates/core/Cargo.toml | head -1 | cut -d'"' -f2)"
if [ "$NEW" == "$OLD" ]; then
    echo "❌ Already at version $OLD"
    exit 1
fi
echo ">> Bumping $OLD → $NEW"

# ---- 1. Pre-flight ---------------------------------------------------------
if [ -n "$(git status --porcelain)" ]; then
    echo "❌ Working tree not clean:"
    git status --porcelain
    exit 1
fi

mapfile -t FILES < <(find crates -name Cargo.toml | sort)
FILES+=(
    packages/npm/package.json
    website/.vitepress/config.ts
    website/public/install.ps1
    plugins.json   # gitignored — bumped for dev mode, never committed
    scripts/release.sh  # usage comment example version
)
for f in "${FILES[@]}"; do
    [ -f "$f" ] || { echo "❌ Missing $f"; exit 1; }
    grep -q "$OLD" "$f" || { echo "❌ Drift: $f does not contain $OLD — fix before releasing"; exit 1; }
done
echo ">> Pre-flight OK: ${#FILES[@]} version-bearing files all at $OLD"

# ---- 2. Bump --------------------------------------------------------------
for f in "${FILES[@]}"; do
    sed -i "s/$OLD/$NEW/g" "$f"
done

# ---- 3. Verify no leftovers ----------------------------------------------
LEFTOVER="$(grep -rn "$OLD" --exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules --exclude-dir=.vitepress --exclude=CHANGELOG.md --exclude=Cargo.lock -l . || true)"
if [ -n "$LEFTOVER" ]; then
    echo "❌ Leftover $OLD in:"; echo "$LEFTOVER"; exit 1
fi
echo ">> Bumped and verified: no leftover $OLD (CHANGELOG.md excluded)"

# ---- 4. Sanity + commit ---------------------------------------------------
cargo check -p santui-core
git add -A
git commit -m "chore: bump version to $NEW"

# ---- 5. Changelog (pre-tag) ----------------------------------------------
git cliff -o CHANGELOG.md
git add -A
git commit -m "chore: update changelog for v$NEW"

# ---- 6. Tag + push --------------------------------------------------------
git tag "v$NEW"
git push origin main "v$NEW"
echo ">> Pushed tag v$NEW — CI building"

# ---- 7. Changelog (post-tag: [unreleased] → v$NEW) ------------------------
git cliff -o CHANGELOG.md
if git diff --quiet CHANGELOG.md; then
    echo ">> Changelog unchanged after tag"
else
    git add -A
    git commit -m "chore: update changelog for v$NEW"
fi
git push origin main

# ---- 8. GitHub release ----------------------------------------------------
NOTES="$(mktemp)"
awk 'BEGIN{f=0} /^## \['"$NEW"'\]/{f=1; next} /^## \[/{if(f) exit} f' CHANGELOG.md > "$NOTES"
gh release create "v$NEW" --notes-file "$NOTES" --title "v$NEW"
rm -f "$NOTES"

echo "✅ Release v$NEW ready: https://github.com/sonyarianto/santui/releases/tag/v$NEW"
echo "   Watch CI: gh run watch --exit-status"
