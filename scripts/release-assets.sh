#!/usr/bin/env bash
set -euo pipefail

die() { echo "release-assets: $*" >&2; exit 1; }

check_app() {
  local app="$1" version="$2" exe archs arch got err
  got=$(plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist" 2>/dev/null) || got="(missing)"
  [[ "$got" == "$version" ]] || die "$app is version $got, not the release version $version"
  exe=$(plutil -extract CFBundleExecutable raw "$app/Contents/Info.plist" 2>/dev/null) || die "$app has no CFBundleExecutable"
  archs=$(lipo -archs "$app/Contents/MacOS/$exe" 2>/dev/null) || die "can't read the architectures of $exe"
  for arch in arm64 x86_64; do
    [[ " $archs " == *" $arch "* ]] || die "$exe is missing $arch (has: $archs); build with --target universal-apple-darwin"
  done
  if ! err=$(codesign --verify --deep --strict "$app" 2>&1); then
    sed 's/^/  /' <<<"$err" >&2
    die "$app: signature doesn't verify (codesign --verify --deep --strict)"
  fi
}

check_archive() {
  local tarball="$1" version="$2" scratch="$3" stray
  stray=$(tar -tzf "$tarball" | grep -vE '^CopilotBar\.app(/|$)' || true)
  [[ -z "$stray" ]] || die "archive has entries outside CopilotBar.app: $stray"
  mkdir -p "$scratch/unpacked"
  tar -xzf "$tarball" -C "$scratch/unpacked"
  check_app "$scratch/unpacked/CopilotBar.app" "$version"
}

check_dmg() {
  local dmg="$1" version="$2" mount="$3/mount" ok=0
  mkdir -p "$mount"
  hdiutil attach -quiet -readonly -nobrowse -noautoopen -mountpoint "$mount" "$dmg" || die "can't open the DMG $dmg"
  ( check_app "$mount/CopilotBar.app" "$version" ) || ok=1
  hdiutil detach -quiet "$mount" || hdiutil detach -quiet -force "$mount" || true
  [[ $ok -eq 0 ]] || die "the DMG's app doesn't pass the checks above"
}

package() {
  local bundle="$1" version="$2" out="$3"
  local app="$bundle/macos/CopilotBar.app"
  [[ -d "$app" ]] || die "no app at $app"
  check_app "$app" "$version"

  SCRATCH=$(mktemp -d)
  trap 'rm -rf "$SCRATCH"' EXIT
  local scratch="$SCRATCH"
  mkdir -p "$scratch/assets"
  # Keep AppleDouble (._) files and xattrs out: check_archive allows only CopilotBar.app entries.
  COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf "$scratch/assets/CopilotBar.app.tar.gz" -C "$bundle/macos" CopilotBar.app
  mkdir -p "$scratch/stage"
  cp -R "$app" "$scratch/stage/"
  ln -s /Applications "$scratch/stage/Applications"
  hdiutil create -quiet -volname CopilotBar -srcfolder "$scratch/stage" -format UDZO \
    "$scratch/assets/CopilotBar_${version}_universal.dmg" || die "hdiutil couldn't make the DMG"
  check_archive "$scratch/assets/CopilotBar.app.tar.gz" "$version" "$scratch"
  check_dmg "$scratch/assets/CopilotBar_${version}_universal.dmg" "$version" "$scratch"

  mkdir -p "$out"
  mv "$scratch/assets/"* "$out/"
  echo "Wrote $out/CopilotBar.app.tar.gz and $out/CopilotBar_${version}_universal.dmg"
}

checksums() {
  local out="$1" f name dmgs=0
  [[ -d "$out" ]] || die "no directory $out"
  for name in CopilotBar.app.tar.gz CopilotBar.app.tar.gz.sig; do
    [[ -f "$out/$name" ]] || die "$out has no $name"
  done
  for f in "$out"/* "$out"/.[!.]*; do
    [[ -e "$f" ]] || continue
    name=$(basename "$f")
    case "$name" in
      CopilotBar.app.tar.gz | CopilotBar.app.tar.gz.sig | SHA256SUMS) ;;
      CopilotBar_*_universal.dmg) dmgs=$((dmgs + 1)) ;;
      *) die "$out has $name, which isn't a release asset" ;;
    esac
  done
  [[ $dmgs -eq 1 ]] || die "expected one DMG in $out, found $dmgs"
  (cd "$out" && LC_ALL=C shasum -a 256 CopilotBar.app.tar.gz CopilotBar.app.tar.gz.sig CopilotBar_*_universal.dmg > SHA256SUMS.tmp && mv SHA256SUMS.tmp SHA256SUMS)
  cat "$out/SHA256SUMS"
}

case "${1:-}" in
  package) [[ $# -eq 4 ]] || die "usage: release-assets.sh package <bundle-dir> <version> <out-dir>"; package "$2" "$3" "$4" ;;
  checksums) [[ $# -eq 2 ]] || die "usage: release-assets.sh checksums <out-dir>"; checksums "$2" ;;
  *) die "usage: release-assets.sh package <bundle-dir> <version> <out-dir> | checksums <out-dir>" ;;
esac
