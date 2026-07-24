#!/usr/bin/env sh
set -e

REPO="sonyarianto/santui"
DEST="${HOME}/.local/share/santui"
BIN_DIR="${DEST}/current"

# ── detect arch ──
ARCH="$(uname -m)"
OS="$(uname -s)"

case "${OS}" in
  Darwin)
    case "${ARCH}" in
      arm64|aarch64) TRIPLE="aarch64-apple-darwin" ;;
      x86_64)        TRIPLE="x86_64-apple-darwin"  ;;
      *)
        echo "Unsupported architecture: ${ARCH}"
        echo "  Build from source: https://github.com/sonyarianto/santui"
        exit 1
        ;;
    esac
    # libmpv and all its transitive dylib deps (libavcodec, libavformat,
    # etc.) are bundled in the release archive with @loader_path-relative
    # paths — no Homebrew required.  The fallback in player.rs additionally
    # checks Homebrew paths, so brew install mpv is optional but nice.
    if ! brew list mpv 2>/dev/null >/dev/null; then
      echo "  Tip: Install mpv for the best compatibility:"
      echo "    brew install mpv"
    fi
    ;;
  Linux)
    case "${ARCH}" in
      x86_64) TRIPLE="x86_64-unknown-linux-gnu" ;;
      *)
        echo "Unsupported architecture: ${ARCH} (only x86_64 is available as a pre-built binary)"
        echo "  Build from source: https://github.com/sonyarianto/santui"
        exit 1
        ;;
    esac
    # check for libmpv
    if ! ldconfig -p 2>/dev/null | grep -q libmpv; then
      echo "  [!] libmpv not found. Install it: sudo apt install libmpv-dev (Debian) or sudo dnf install mpv-libs-devel (Fedora)"
    fi
    ;;
  *)
    echo "Unsupported OS: ${OS}"
    exit 1
    ;;
esac

echo ">> Fetching latest release..."
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
TAG="$(curl -sSfL "${API_URL}" | grep '"tag_name"' | cut -d'"' -f4)"
ARCHIVE_URL="https://github.com/${REPO}/releases/download/${TAG}/santui-${TRIPLE}.tar.gz"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

echo ">> Downloading santui (${TRIPLE})..."
curl -fsSL "${ARCHIVE_URL}" -o "${TMP}/santui.tar.gz"

echo "  Extracting..."
tar xzf "${TMP}/santui.tar.gz" -C "${TMP}"

mkdir -p "${BIN_DIR}"
cp -r "${TMP}/"* "${BIN_DIR}/"
chmod +x "${BIN_DIR}/santui" "${BIN_DIR}"/santui-* 2>/dev/null || true

# ── PATH ──
UPDATED=""
case ":${PATH}:" in
  *":${BIN_DIR}:"*) ;;
  *)
    echo "export PATH=\"${BIN_DIR}:\$PATH\"" >> "${HOME}/.bashrc"
    if [ -f "${HOME}/.zshrc" ]; then
      echo "export PATH=\"${BIN_DIR}:\$PATH\"" >> "${HOME}/.zshrc"
    fi
    export PATH="${BIN_DIR}:${PATH}"
    UPDATED="yes"
    ;;
esac

echo "[OK] Installed to ${BIN_DIR}"
if [ -n "${UPDATED}" ]; then
  echo "  Restart your terminal or run: export PATH=\"${BIN_DIR}:\$PATH\""
fi
echo "  Run: santui"
