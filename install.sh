#!/bin/sh
# CopilotBar installer for macOS. Read it before you run it:
#   curl -fsSL https://raw.githubusercontent.com/kaanakdeniz/copilot-bar/main/install.sh -o install.sh
#   less install.sh && sh install.sh
set -eu

REPO=kaanakdeniz/copilot-bar
ARCHIVE=CopilotBar.app.tar.gz
APP=CopilotBar.app
TRUSTED_GH_DIRS="/opt/homebrew/bin /usr/local/bin /usr/bin"

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }

find_gh() {
  for dir in $TRUSTED_GH_DIRS; do
    if [ -x "$dir/gh" ]; then
      echo "$dir/gh"
      return 0
    fi
  done
  return 1
}

install_dir() {
  if [ -n "${COPILOTBAR_INSTALL_DIR:-}" ]; then
    echo "$COPILOTBAR_INSTALL_DIR"
  elif [ -w /Applications ]; then
    echo /Applications
  else
    echo "$HOME/Applications"
  fi
}

main() {
  [ "$(uname -s)" = Darwin ] || fail "CopilotBar runs on macOS only."

  gh=$(find_gh) || fail "CopilotBar needs the GitHub CLI. Install it with: brew install gh"
  "$gh" auth status --hostname github.com >/dev/null 2>&1 ||
    fail "Sign in to the GitHub CLI first: gh auth login"

  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT

  say "Downloading the latest release"
  "$gh" release download --repo "github.com/$REPO" --pattern "$ARCHIVE" --dir "$work" ||
    fail "Download failed."

  say "Checking that it was built by this repository's release workflow"
  "$gh" attestation verify "$work/$ARCHIVE" --repo "$REPO" --hostname github.com >/dev/null ||
    fail "The download failed its provenance check. Nothing was installed."

  tar -xzf "$work/$ARCHIVE" -C "$work" || fail "The archive could not be unpacked."
  [ -d "$work/$APP" ] || fail "The archive has no $APP."

  dest=$(install_dir)
  mkdir -p "$dest"
  pkill -f "$dest/$APP/Contents/MacOS/" 2>/dev/null || true
  rm -rf "${dest:?}/$APP"
  mv "$work/$APP" "$dest/"
  say "Installed $dest/$APP"

  if [ -z "${COPILOTBAR_NO_OPEN:-}" ]; then
    open "$dest/$APP"
    say "Look for the Copilot icon in your menu bar."
  fi
}

main "$@"
