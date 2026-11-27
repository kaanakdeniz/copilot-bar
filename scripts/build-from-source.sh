#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

say() { printf "\033[1m==>\033[0m %s\n" "$*"; }
need() { command -v "$1" >/dev/null 2>&1; }

[[ "$(uname)" == "Darwin" ]] || { echo "This installer is for macOS. CopilotBar is macOS-only."; exit 1; }

# Homebrew's rustup is keg-only, so its bin directory is not on PATH.
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
if ! need cargo; then
  if need brew; then say "Installing Rust (brew install rustup)"; brew install rustup >/dev/null; rustup default stable
  else echo "Rust is required: https://rustup.rs"; exit 1; fi
fi
need node || { echo "Node.js 22.12+ is required: brew install node"; exit 1; }
xcode-select -p >/dev/null 2>&1 || { echo "Xcode Command Line Tools are required: xcode-select --install"; exit 1; }

say "Installing JavaScript dependencies"
npm ci --ignore-scripts --no-audit --no-fund --loglevel=error

say "Building CopilotBar (first build takes a few minutes)"
LOG="$(mktemp -t copilotbar-build)"
if ! npx tauri build --bundles app >"$LOG" 2>&1; then
  echo "Build failed. Full log: $LOG"; tail -40 "$LOG"; exit 1
fi
grep -E "Finished|Built application" "$LOG" || true
APP="src-tauri/target/release/bundle/macos/CopilotBar.app"

say "Installing to /Applications"
pkill -f "CopilotBar.app/Contents/MacOS/copilot-bar" 2>/dev/null || true
rm -rf /Applications/CopilotBar.app
cp -R "$APP" /Applications/
xattr -dr com.apple.quarantine /Applications/CopilotBar.app 2>/dev/null || true

if [[ "${1:-}" != "--no-open" ]]; then
  say "Launching. Look for the Copilot icon in your menu bar."
  open /Applications/CopilotBar.app
fi
