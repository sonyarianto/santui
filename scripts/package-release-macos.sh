#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="$ROOT/target/release"
OUTDIR="$ROOT/releases"

VERSION="${1:-nightly}"

# ── detect arch ──
ARCH="$(uname -m)"
case "$ARCH" in
  arm64|aarch64) TRIPLE="aarch64-apple-darwin" ;;
  x86_64)        TRIPLE="x86_64-apple-darwin"  ;;
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
cp "$TARGET/santui-radio-streaming-player" "$STAGE/"
cp "$ROOT/native/radio_streaming_stations.db" "$STAGE/native/"

echo "  Bundling libmpv and transitive dylib deps …"

NATIVE="$STAGE/native"

# Locate Homebrew prefix (works on both Apple Silicon & Intel)
HOMEBREW_PREFIX="$(brew --prefix 2>/dev/null || echo /opt/homebrew)"
LIBMPV_SRC="$HOMEBREW_PREFIX/lib/libmpv.2.dylib"

if [ ! -f "$LIBMPV_SRC" ]; then
  echo "  [!] libmpv.2.dylib not found at $LIBMPV_SRC"
  echo "  Install mpv first via Homebrew: brew install mpv"
  exit 1
fi

# Bundle libmpv itself
cp "$LIBMPV_SRC" "$NATIVE/"

# ── Recursively bundle all transitive Homebrew dylib deps ──
# Homebrew dylibs embed absolute LC_LOAD_DYLIB paths pointing to
# $HOMEBREW_PREFIX/lib/.  We copy every dependency & rewrite its
# path to @loader_path/<name> so the bundle is relocatable to any
# machine (no Homebrew required).
bundle_deps() {
  local dylib="$1"
  local deps
  deps="$(otool -L "$dylib" 2>/dev/null \
    | tail -n +2 \
    | grep -E "^[[:space:]]+($HOMEBREW_PREFIX|/usr/local)/lib" \
    | awk '{print $1}' \
    || true)"
  for dep in $deps; do
    local name; name="$(basename "$dep")"
    local target="$NATIVE/$name"
    if [ ! -f "$target" ]; then
      cp "$dep" "$target"
      install_name_tool -id "@loader_path/$name" "$target" 2>/dev/null || true
      bundle_deps "$target"
    fi
    install_name_tool -change "$dep" "@loader_path/$name" "$dylib" 2>/dev/null || true
  done
}

bundle_deps "$NATIVE/libmpv.2.dylib"

echo "  Collected $(ls -1 "$NATIVE"/*.dylib 2>/dev/null | wc -l | tr -d ' ') dylibs"
echo ""

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
tar tzf "$ARCHIVE_PATH" | head -20
echo "  … $(tar tzf "$ARCHIVE_PATH" | wc -l | tr -d ' ') files total"
echo ""

# ── clean stage ──
echo "  Cleaning up …"
rm -rf "$(dirname "$STAGE")"

# ── done ──
SIZE="$(du -h "$ARCHIVE_PATH" | cut -f1)"
echo "✔ $ARCHIVE_NAME — ${SIZE}B"
