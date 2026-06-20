#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTDIR="$ROOT/target/debug"

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
        santui.exe|santui|santui-*-scraper|santui-*-scraper.exe) continue ;;
    esac

    id="${name#santui-}"
    id="${id%.exe}"
    size=$(stat -f%z "$bin" 2>/dev/null || stat -c%s "$bin" 2>/dev/null)
    hash=$($SHA_CMD "$bin" | cut -d' ' -f1)

    echo "  [OK] $id  ($size bytes, sha256=$hash)"
    PLUGINS+=("$(cat << JSON
{"id":"$id","name":"Radio Streaming Player","description":"Listen to 50,000+ radio stations","version":"0.2.1","download_url":"target/debug/$name","sha256":"$hash","size":$size}
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