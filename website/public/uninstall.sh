#!/usr/bin/env sh
set -e

DEST="${HOME}/.local/share/santui"
BIN_DIR="${DEST}/current"

echo ">> Uninstalling santui ..."

# ── remove files ──
if [ -d "${DEST}" ]; then
  rm -rf "${DEST}"
  echo "  Removed ${DEST}"
else
  echo "  ${DEST} not found — skipping"
fi

# ── remove from shell configs ──
SHELL_LINE="export PATH=\"${BIN_DIR}:\$PATH\""

for RC in "${HOME}/.bashrc" "${HOME}/.zshrc"; do
  if [ -f "${RC}" ]; then
    # remove only the exact line that the installer added
    if grep -qF "${SHELL_LINE}" "${RC}" 2>/dev/null; then
      # create a temp file without that line
      grep -vF "${SHELL_LINE}" "${RC}" > "${RC}.tmp" 2>/dev/null || true
      if [ -f "${RC}.tmp" ]; then
        mv "${RC}.tmp" "${RC}"
        echo "  Cleaned ${RC}"
      fi
    fi
  fi
done

echo ""
echo "[OK] Santui has been uninstalled."
echo "  Restart your terminal or run: source ~/.bashrc (or source ~/.zshrc)"
