#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$ROOT/scripts/release-assets.sh"
FAILED=0
PASSED=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

BIN_UNIVERSAL="$WORK/bin-universal"
BIN_ARM64="$WORK/bin-arm64"
echo 'int main(void) { return 0; }' > "$WORK/main.c"
clang -arch arm64 -arch x86_64 -o "$BIN_UNIVERSAL" "$WORK/main.c" || { echo "clang failed"; exit 1; }
clang -arch arm64 -o "$BIN_ARM64" "$WORK/main.c" || { echo "clang failed"; exit 1; }

new_bundle() {
  local bin="${1:-$BIN_UNIVERSAL}" version="${2:-0.3.0}" dir app
  dir=$(mktemp -d "$WORK/bundle.XXXXXX")
  app="$dir/macos/CopilotBar.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$bin" "$app/Contents/MacOS/copilot-bar"
  echo "icon" > "$app/Contents/Resources/icon.icns"
  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>copilot-bar</string>
  <key>CFBundleIdentifier</key><string>dev.copilotbar</string>
  <key>CFBundleName</key><string>CopilotBar</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
</dict>
</plist>
PLIST
  codesign --force --deep -s - "$app" 2>/dev/null || { echo "codesign failed" >&2; return 1; }
  echo "$dir"
}

pass() { echo "ok   - $1"; }
fail() { echo "FAIL - $1"; shift; for line in "$@"; do echo "       $line"; done; }

run() { OUT=$("$SCRIPT" "$@" 2>&1); STATUS=$?; }

listing() { (cd "$1" && LC_ALL=C ls -1A | tr '\n' ' '); }

test_package_writes_the_archive_and_the_dmg() {
  local name="package writes CopilotBar.app.tar.gz and the universal DMG, nothing else"
  local bundle out
  bundle=$(new_bundle) || { fail "$name" "fixture failed"; return; }
  out="$WORK/out-package"
  run package "$bundle" 0.3.0 "$out"
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  local got
  got=$(listing "$out")
  if [[ "$got" != "CopilotBar.app.tar.gz CopilotBar_0.3.0_universal.dmg " ]]; then fail "$name" "assets: $got"; return; fi
  pass "$name"
}

test_archive_unpacks_to_one_signed_app() {
  local name="the archive holds only CopilotBar.app, which unpacks with a valid signature"
  local bundle out dest
  bundle=$(new_bundle) || { fail "$name" "fixture failed"; return; }
  out="$WORK/out-layout"
  run package "$bundle" 0.3.0 "$out"
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  local stray
  stray=$(tar -tzf "$out/CopilotBar.app.tar.gz" | grep -vE '^CopilotBar\.app(/|$)')
  if [[ -n "$stray" ]]; then fail "$name" "entries outside CopilotBar.app:" "$stray"; return; fi
  if tar -tzf "$out/CopilotBar.app.tar.gz" | grep -q '/\._'; then fail "$name" "AppleDouble ._ files in the archive"; return; fi
  dest="$WORK/unpacked"
  mkdir -p "$dest" && tar -xzf "$out/CopilotBar.app.tar.gz" -C "$dest"
  if ! codesign --verify --deep --strict "$dest/CopilotBar.app" 2>/dev/null; then
    fail "$name" "unpacked app fails codesign --verify --deep --strict"; return
  fi
  pass "$name"
}

test_dmg_holds_the_signed_app_and_an_applications_link() {
  local name="the DMG holds the signed app and a link to /Applications"
  local bundle out mount
  bundle=$(new_bundle) || { fail "$name" "fixture failed"; return; }
  out="$WORK/out-dmg"
  run package "$bundle" 0.3.0 "$out"
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  mount=$(mktemp -d "$WORK/mount.XXXXXX")
  hdiutil attach -quiet -readonly -nobrowse -mountpoint "$mount" "$out/CopilotBar_0.3.0_universal.dmg" || { fail "$name" "can't attach"; return; }
  local problem=""
  [[ "$(readlink "$mount/Applications")" == "/Applications" ]] || problem="no Applications link"
  codesign --verify --deep --strict "$mount/CopilotBar.app" 2>/dev/null || problem="app in the DMG fails codesign"
  hdiutil detach -quiet "$mount" || hdiutil detach -quiet -force "$mount"
  if [[ -n "$problem" ]]; then fail "$name" "$problem"; return; fi
  pass "$name"
}

expect_package_refused() {
  local name="$1" pattern="$2" bundle="$3" version="${4:-0.3.0}" out
  out=$(mktemp -d "$WORK/out.XXXXXX")
  run package "$bundle" "$version" "$out"
  if [[ $STATUS -eq 0 ]]; then fail "$name" "expected a non-zero exit" "$OUT"; return; fi
  if ! grep -qiE "$pattern" <<<"$OUT"; then fail "$name" "message didn't match /$pattern/:" "$OUT"; return; fi
  if [[ -n "$(listing "$out")" ]]; then fail "$name" "assets left behind: $(listing "$out")"; return; fi
  pass "$name"
}

test_refuses_a_single_architecture_app() {
  local bundle
  bundle=$(new_bundle "$BIN_ARM64") || { fail "single-arch fixture failed"; return; }
  expect_package_refused "refuses an app without an x86_64 slice" "x86_64" "$bundle"
}

test_refuses_an_app_changed_after_signing() {
  local bundle
  bundle=$(new_bundle) || { fail "tamper fixture failed"; return; }
  echo "changed" > "$bundle/macos/CopilotBar.app/Contents/Resources/icon.icns"
  expect_package_refused "refuses an app whose signature no longer verifies" "signature|codesign" "$bundle"
}

test_refuses_an_unsigned_app() {
  local bundle
  bundle=$(new_bundle) || { fail "unsigned fixture failed"; return; }
  codesign --remove-signature "$bundle/macos/CopilotBar.app"
  expect_package_refused "refuses an unsigned app" "signature|codesign" "$bundle"
}

test_refuses_an_app_of_another_version() {
  local bundle
  bundle=$(new_bundle "$BIN_UNIVERSAL" 0.2.0) || { fail "version fixture failed"; return; }
  expect_package_refused "refuses an app whose version isn't the release version" "0\.2\.0.*0\.3\.0|0\.3\.0.*0\.2\.0" "$bundle"
}

packaged_out() {
  local bundle out
  bundle=$(new_bundle) || return 1
  out=$(mktemp -d "$WORK/out.XXXXXX")
  "$SCRIPT" package "$bundle" 0.3.0 "$out" >/dev/null 2>&1 || return 1
  echo "stand-in signature" > "$out/CopilotBar.app.tar.gz.sig"
  echo "$out"
}

test_checksums_cover_exactly_the_other_assets() {
  local name="checksums writes SHA256SUMS for the archive, its signature and the DMG"
  local out
  out=$(packaged_out) || { fail "$name" "fixture failed"; return; }
  run checksums "$out"
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  local names
  names=$(awk '{print $2}' "$out/SHA256SUMS" | tr '\n' ' ')
  if [[ "$names" != "CopilotBar.app.tar.gz CopilotBar.app.tar.gz.sig CopilotBar_0.3.0_universal.dmg " ]]; then
    fail "$name" "SHA256SUMS lists: $names"; return
  fi
  if ! (cd "$out" && shasum -a 256 -c SHA256SUMS >/dev/null 2>&1); then fail "$name" "shasum -c fails:" "$(cat "$out/SHA256SUMS")"; return; fi
  if [[ "$(listing "$out")" != "CopilotBar.app.tar.gz CopilotBar.app.tar.gz.sig CopilotBar_0.3.0_universal.dmg SHA256SUMS " ]]; then
    fail "$name" "assets: $(listing "$out")"; return
  fi
  pass "$name"
}

test_checksums_refuses_a_missing_signature() {
  local name="checksums refuses an <out-dir> without the signature"
  local out
  out=$(packaged_out) || { fail "$name" "fixture failed"; return; }
  rm "$out/CopilotBar.app.tar.gz.sig"
  run checksums "$out"
  if [[ $STATUS -eq 0 ]] || ! grep -q "CopilotBar.app.tar.gz.sig" <<<"$OUT"; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  if [[ -e "$out/SHA256SUMS" ]]; then fail "$name" "SHA256SUMS written anyway"; return; fi
  pass "$name"
}

test_checksums_refuses_a_stray_file() {
  local name="checksums refuses an <out-dir> with anything besides the release assets"
  local out
  out=$(packaged_out) || { fail "$name" "fixture failed"; return; }
  echo "x" > "$out/notes.txt"
  run checksums "$out"
  if [[ $STATUS -eq 0 ]] || ! grep -q "notes.txt" <<<"$OUT"; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  if [[ -e "$out/SHA256SUMS" ]]; then fail "$name" "SHA256SUMS written anyway"; return; fi
  pass "$name"
}

for t in $(declare -F | awk '{print $3}' | grep '^test_'); do
  ( "$t" ) | tee "$WORK/out"
  grep -q '^FAIL' "$WORK/out" && FAILED=$((FAILED + 1)) || PASSED=$((PASSED + 1))
done

echo "$PASSED passed, $FAILED failed"
[[ $FAILED -eq 0 ]]
