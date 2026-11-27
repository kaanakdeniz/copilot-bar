#!/usr/bin/env bash
# Tauri reads APPLE_SIGNING_IDENTITY, APPLE_ID, APPLE_PASSWORD and APPLE_TEAM_ID to sign and notarize.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
npm ci --no-audit --no-fund --loglevel=error
if [[ -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "note: APPLE_SIGNING_IDENTITY not set; building an unsigned (ad-hoc) DMG"
fi
# The styled DMG drives Finder, which needs Automation permission; the fallback is a plain DMG.
if ! npx tauri build --bundles app,dmg; then
  echo "note: styled DMG failed (Finder automation not allowed); building a plain DMG"
  VERSION=$(node -p "require('./package.json').version")
  ARCH=$(uname -m)
  STAGE=$(mktemp -d)
  cp -R src-tauri/target/release/bundle/macos/CopilotBar.app "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  mkdir -p src-tauri/target/release/bundle/dmg
  OUT="src-tauri/target/release/bundle/dmg/CopilotBar_${VERSION}_${ARCH}.dmg"
  rm -f "$OUT"
  hdiutil create -volname CopilotBar -srcfolder "$STAGE" -ov -format UDZO "$OUT" >/dev/null
  rm -rf "$STAGE"
fi
ls -1 src-tauri/target/release/bundle/dmg/*.dmg
