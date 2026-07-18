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
    echo ">> Skipping workspace build (--no-build)"
    if [ ! -d "$OUTDIR" ]; then
        echo "error: $OUTDIR does not exist — run without --no-build first" >&2
        exit 1
    fi
else
    echo ">> Building workspace (debug) ..."
    cargo build --workspace
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

python3 -c "
import json, hashlib, os, glob, sys

root = os.environ['ROOT']
outdir = os.environ['OUTDIR']
version = os.environ['VERSION']

with open(os.path.join(root, 'plugins-manifest.json')) as f:
    manifest = {p['id']: p for p in json.load(f)}

plugins = []
for binpath in sorted(glob.glob(os.path.join(outdir, 'santui-*'))):
    name = os.path.basename(binpath)
    if name in ('santui', 'santui.exe'):
        continue
    if '-scraper' in name or 'registry-plugin' in name:
        continue
    stem = name[:-4] if name.endswith('.exe') else name
    pid = stem[len('santui-'):]
    p = manifest.get(pid)
    if not p:
        continue
    sha = hashlib.sha256()
    with open(binpath, 'rb') as bf:
        while True:
            chunk = bf.read(65536)
            if not chunk:
                break
            sha.update(chunk)
    size = os.path.getsize(binpath)
    plugins.append({
        'id': pid,
        'name': p['name'],
        'publisher': 'Santui',
        'description': p['description'],
        'version': version,
        'download_url': f'target/debug/{name}',
        'sha256': sha.hexdigest(),
        'size': size,
        'capabilities': p.get('capabilities', [])
    })
    print(f'  [OK] {pid}  ({size} bytes, sha256={sha.hexdigest()})')

with open(os.path.join(root, 'plugins.json'), 'w') as f:
    json.dump(plugins, f, indent=2)

s = 's' if len(plugins) != 1 else ''
print(f'[OK] plugins.json generated ({len(plugins)} plugin{s})')
"
