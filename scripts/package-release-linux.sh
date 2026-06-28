#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="$ROOT/target/release"
OUTDIR="$ROOT/releases"

VERSION="${1:-nightly}"

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) TRIPLE="x86_64-unknown-linux-gnu"  ;;
  aarch64) TRIPLE="aarch64-unknown-linux-gnu" ;;
  *) echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

# ── build ──
echo "» Building release binaries …"
cargo build --release --workspace
echo ""

# ── stage ──
STAGE="$(mktemp -d)/santui-pkg/$VERSION"
mkdir -p "$STAGE/native"

cp "$TARGET/santui"                       "$STAGE/"
cp "$TARGET/santui-registry-plugin"       "$STAGE/"
cp "$TARGET/santui-radio-stream-player" "$STAGE/"
cp "$ROOT/native/radio_stream_stations.db" "$STAGE/native/"

# ── archive ──
ARCHIVE_NAME="santui-${TRIPLE}.tar.gz"
ARCHIVE_PATH="$OUTDIR/$ARCHIVE_NAME"
mkdir -p "$OUTDIR"
rm -f "$ARCHIVE_PATH"

echo "  Packing $ARCHIVE_NAME …"
tar czf "$ARCHIVE_PATH" -C "$STAGE" .
echo ""

# ── verify ──
echo "  Archive contents:"
tar tzf "$ARCHIVE_PATH" | head -10
echo "  … $(tar tzf "$ARCHIVE_PATH" | wc -l | tr -d ' ') files total"
echo ""

# ── clean stage ──
rm -rf "$(dirname "$STAGE")"

# ── done ──
SIZE="$(du -h "$ARCHIVE_PATH" | cut -f1)"
echo "✔ $ARCHIVE_NAME — ${SIZE}B"
