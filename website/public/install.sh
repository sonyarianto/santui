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
      x86_64) TRIPLE="x86_64-apple-darwin" ;;
      arm64|aarch64) TRIPLE="aarch64-apple-darwin" ;;
      *) echo "Unsupported architecture: ${ARCH}"; exit 1 ;;
    esac
    # ensure mpv is installed
    if ! command -v mpv >/dev/null 2>&1; then
      if command -v brew >/dev/null 2>&1; then
        echo ">> Installing mpv via Homebrew..."
        brew install mpv
      else
        echo "  [!] mpv not found. Install it first: brew install mpv"
      fi
    fi
    ;;
  Linux)
    case "${ARCH}" in
      x86_64) TRIPLE="x86_64-unknown-linux-gnu" ;;
      aarch64) TRIPLE="aarch64-unknown-linux-gnu" ;;
      *) echo "Unsupported architecture: ${ARCH}"; exit 1 ;;
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
TAG="$(curl -sfL "${API_URL}" | grep '"tag_name"' | cut -d'"' -f4)"
ZIP_URL="https://github.com/${REPO}/releases/download/${TAG}/santui-${TRIPLE}.zip"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

echo ">> Downloading santui (${TRIPLE})..."
curl -fsSL "${ZIP_URL}" -o "${TMP}/santui.zip"

echo "  Extracting..."
unzip -qo "${TMP}/santui.zip" -d "${TMP}/extracted"

mkdir -p "${BIN_DIR}"
cp -r "${TMP}/extracted/"* "${BIN_DIR}/"
chmod +x "${BIN_DIR}/santui" "${BIN_DIR}/santui-radio-streaming-player" 2>/dev/null || true

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
