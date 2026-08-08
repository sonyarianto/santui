#!/usr/bin/env bash
# Fast local verification: host + core crates + stable plugins only.
#
# Full-workspace verification (all ~127 crates, incl. experimental plugins)
# is CI's job (.github/workflows/ci.yml). Run `cargo check/clippy/test
# --workspace` manually only when you are working on an experimental plugin.
#
# Usage: scripts/check.sh [check|clippy|test|fmt]
set -euo pipefail
cd "$(dirname "$0")/.."

PKGS=(
    -p santui
    -p santui-core
    -p santui-ipc
    -p santui-auth
    -p santui-registry
    -p santui-db
    -p santui-server
    -p santui-dev-setup
    -p santui-registry-plugin
    -p santui-log-viewer
)
for id in $(cargo run -q -p santui-dev-setup -- list-ids); do
    PKGS+=(-p "santui-$id")
done

case "${1:-check}" in
    check) cargo check "${PKGS[@]}" ;;
    clippy) cargo clippy "${PKGS[@]}" -- -D warnings ;;
    test) cargo test "${PKGS[@]}" ;;
    fmt) cargo fmt --check ;;
    *)
        echo "usage: $0 [check|clippy|test|fmt]" >&2
        exit 2
        ;;
esac
