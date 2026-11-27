#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAILED=0
PASSED=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

STUB_DIR="$WORK/gh-bin"
mkdir -p "$STUB_DIR"
cat > "$STUB_DIR/gh" <<'STUB'
#!/bin/sh
echo "$*" >> "$GH_CALLS"
case "$1 $2" in
  "auth status") exit "${GH_AUTH_STATUS:-0}" ;;
  "attestation verify") exit "${GH_ATTESTATION_STATUS:-0}" ;;
  "release download")
    while [ $# -gt 0 ]; do
      [ "$1" = --dir ] && dir=$2
      shift
    done
    cp "$GH_ARCHIVE" "$dir/CopilotBar.app.tar.gz"
    ;;
esac
STUB
chmod +x "$STUB_DIR/gh"

INSTALLER="$WORK/install.sh"
sed "s|^TRUSTED_GH_DIRS=.*|TRUSTED_GH_DIRS=\"\$TEST_GH_DIRS\"|" "$ROOT/install.sh" > "$INSTALLER"
grep -q '^TRUSTED_GH_DIRS="\$TEST_GH_DIRS"$' "$INSTALLER" || { echo "could not point the installer at the stub gh"; exit 1; }

archive_with() {
  local name="$1" src
  src=$(mktemp -d "$WORK/src.XXXXXX")
  mkdir -p "$src/$name/Contents/MacOS"
  echo new > "$src/$name/Contents/MacOS/copilot-bar"
  tar -czf "$src.tar.gz" -C "$src" "$name"
  echo "$src.tar.gz"
}
GOOD_ARCHIVE=$(archive_with CopilotBar.app)
WRONG_ARCHIVE=$(archive_with Other.app)

pass() { echo "ok   - $1"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL - $1"; shift; for line in "$@"; do echo "       $line"; done; FAILED=$((FAILED + 1)); }

setup() {
  DEST=$(mktemp -d "$WORK/dest.XXXXXX")
  export GH_CALLS="$WORK/calls.$RANDOM"
  : > "$GH_CALLS"
  mkdir -p "$DEST/CopilotBar.app/Contents/MacOS"
  echo old > "$DEST/CopilotBar.app/Contents/MacOS/copilot-bar"
}

run() {
  OUT=$(env TEST_GH_DIRS="${TEST_GH_DIRS:-$STUB_DIR}" GH_ARCHIVE="${GH_ARCHIVE:-$GOOD_ARCHIVE}" \
    COPILOTBAR_INSTALL_DIR="$DEST" COPILOTBAR_NO_OPEN=1 "$@" sh "$INSTALLER" 2>&1)
  STATUS=$?
}

installed() { cat "$DEST/CopilotBar.app/Contents/MacOS/copilot-bar" 2>/dev/null; }

test_installs_a_verified_release() {
  local name="installs the verified release over the old app"
  setup; run
  if [[ $STATUS -eq 0 && "$(installed)" == new && "$OUT" == *"Installed $DEST/CopilotBar.app"* ]] &&
     grep -qx "attestation verify .*/CopilotBar.app.tar.gz --repo kaanakdeniz/copilot-bar --hostname github.com" "$GH_CALLS" &&
     grep -qx "release download --repo github.com/kaanakdeniz/copilot-bar --pattern CopilotBar.app.tar.gz --dir .*" "$GH_CALLS"; then
    pass "$name"
  else
    fail "$name" "status=$STATUS installed=$(installed)" "$OUT" "$(cat "$GH_CALLS")"
  fi
}

test_failed_attestation_keeps_the_old_app() {
  local name="a failed provenance check installs nothing"
  setup; run GH_ATTESTATION_STATUS=1
  if [[ $STATUS -ne 0 && "$(installed)" == old && "$OUT" == *"failed its provenance check. Nothing was installed."* ]]; then
    pass "$name"
  else
    fail "$name" "status=$STATUS installed=$(installed)" "$OUT"
  fi
}

test_archive_without_the_app_keeps_the_old_app() {
  local name="an archive without CopilotBar.app installs nothing"
  setup; GH_ARCHIVE="$WRONG_ARCHIVE" run
  if [[ $STATUS -ne 0 && "$(installed)" == old && "$OUT" == *"The archive has no CopilotBar.app."* ]]; then
    pass "$name"
  else
    fail "$name" "status=$STATUS installed=$(installed)" "$OUT"
  fi
}

test_signed_out_gh_stops_before_downloading() {
  local name="a signed-out gh stops before any download"
  setup; run GH_AUTH_STATUS=1
  if [[ $STATUS -ne 0 && "$(installed)" == old && "$OUT" == *"Sign in to the GitHub CLI first: gh auth login"* ]] &&
     ! grep -q "release download" "$GH_CALLS"; then
    pass "$name"
  else
    fail "$name" "status=$STATUS installed=$(installed)" "$OUT" "$(cat "$GH_CALLS")"
  fi
}

test_gh_outside_the_trusted_dirs_is_not_used() {
  local name="gh outside the trusted folders is not used"
  setup; TEST_GH_DIRS="$WORK/empty" PATH="$STUB_DIR:$PATH" run
  if [[ $STATUS -ne 0 && "$(installed)" == old && "$OUT" == *"brew install gh"* && ! -s "$GH_CALLS" ]]; then
    pass "$name"
  else
    fail "$name" "status=$STATUS installed=$(installed)" "$OUT"
  fi
}

test_installs_a_verified_release
test_failed_attestation_keeps_the_old_app
test_archive_without_the_app_keeps_the_old_app
test_signed_out_gh_stops_before_downloading
test_gh_outside_the_trusted_dirs_is_not_used

echo
echo "$PASSED passed, $FAILED failed"
[[ $FAILED -eq 0 ]]
