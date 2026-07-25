#!/usr/bin/env bash
set -euo pipefail

NO_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --no-build) NO_BUILD=1 ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

export ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export OUTDIR="$ROOT/target/debug"

# Derive version from the single source of truth — crates/core/Cargo.toml
export VERSION="$(grep '^version' "$ROOT/crates/core/Cargo.toml" | head -1 | cut -d'"' -f2)"

if [ "$NO_BUILD" -eq 1 ]; then
    echo ">> Skipping build (--no-build)"
    if [ ! -d "$OUTDIR" ]; then
        echo "error: $OUTDIR does not exist — run without --no-build first" >&2
        exit 1
    fi
else
    echo ">> Building host + dev-setup + builtins ..."
    cargo build -p santui -p santui-dev-setup -p santui-registry-plugin -p santui-log-viewer

    echo ">> Building stable plugins ..."
    STABLE_IDS=$(BINARY_DIR="$OUTDIR" "$OUTDIR/santui-dev-setup" list-ids 2>/dev/null || true)
    if [ -n "$STABLE_IDS" ]; then
        # shellcheck disable=SC2086
        cargo build $(for id in $STABLE_IDS; do echo "-p santui-$id"; done)
    fi
fi

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

"$OUTDIR/santui-dev-setup"
