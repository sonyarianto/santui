#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTDIR="$ROOT/target/debug"

# Derive version from the single source of truth — crates/core/Cargo.toml
VERSION="$(grep '^version' "$ROOT/crates/core/Cargo.toml" | head -1 | cut -d'"' -f2)"

echo ">> Building workspace (debug) ..."
cargo build --workspace

# -- copy native assets --
echo ">> Copying native assets to $OUTDIR/native/ ..."
if [ -d "$ROOT/native" ]; then
    mkdir -p "$OUTDIR/native"
    for f in "$ROOT/native/"*; do
        cp "$f" "$OUTDIR/native/"
        echo "  [OK] $(basename "$f")"
    done
fi

# -- generate plugins.json --
echo ">> Generating plugins.json ..."

# Determine SHA-256 command
if command -v shasum &>/dev/null; then
    SHA_CMD="shasum -a 256"
elif command -v sha256sum &>/dev/null; then
    SHA_CMD="sha256sum"
else
    echo "  [!] No SHA-256 tool found (install coreutils or shasum)"
    exit 1
fi

PLUGINS=()
for bin in "$OUTDIR"/santui-*.exe "$OUTDIR"/santui-*; do
    [ -f "$bin" ] || continue
    name="$(basename "$bin")"
    # Skip the main santui binary and scraper
    case "$name" in
        santui.exe|santui|santui-*-scraper|santui-*-scraper.exe|santui-registry-plugin|santui-registry-plugin.exe) continue ;;
    esac

    id="${name#santui-}"
    id="${id%.exe}"
    size=$(stat -f%z "$bin" 2>/dev/null || stat -c%s "$bin" 2>/dev/null)
    hash=$($SHA_CMD "$bin" | cut -d' ' -f1)

    # Plugin metadata: maps binary id -> (display name, description, capabilities)
    local pname="$id"
    local pdesc="$id"
    local pcaps="[]"
    case "$id" in
        radio-stream-player)
            pname="Radio Stream Player"
            pdesc="Listen to thousands of radio stations worldwide"
            pcaps='["background"]'
            ;;
        system-monitor)
            pname="System Monitor"
            pdesc="Real-time CPU, memory, disk, network, and process monitor"
            pcaps='[]'
            ;;
        world-clock)
            pname="World Clock"
            pdesc="World timezone clock with grid, detail view, search, and custom labels"
            pcaps='[]'
            ;;
        weather)
            pname="Weather"
            pdesc="Current conditions, hourly & 7-day forecast, location search, auto-refresh"
            pcaps='[]'
            ;;
        rss-reader)
            pname="RSS Reader"
            pdesc="Subscribe to and read RSS/Atom feeds"
            pcaps='[]'
            ;;
        clipboard-history)
            pname="Clipboard History"
            pdesc="Track and search clipboard history"
            pcaps='[]'
            ;;
        hacker-news-reader)
            pname="Hacker News Reader"
            pdesc="Browse top, new, and best stories from Hacker News"
            pcaps='[]'
            ;;
    esac

    echo "  [OK] $id  ($size bytes, sha256=$hash)"
    PLUGINS+=("$(cat << JSON
{"id":"$id","name":"$pname","publisher":"Santui","description":"$pdesc","version":"$VERSION","download_url":"target/debug/$name","sha256":"$hash","size":$size,"capabilities":$pcaps}
JSON
)")
done

# Build JSON array
JOINED=""
for p in "${PLUGINS[@]}"; do
    if [ -n "$JOINED" ]; then
        JOINED="$JOINED,$p"
    else
        JOINED="$p"
    fi
done
JSON="[$JOINED]"

printf '%s' "$JSON" > "$ROOT/plugins.json"
count="${#PLUGINS[@]}"
echo "[OK] plugins.json generated ($count plugin$( [ "$count" -ne 1 ] && echo 's' ))"
