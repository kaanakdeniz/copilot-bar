#!/usr/bin/env bash
# Tauri's signature format; the trusted comment binds the version.
# Signer fetched at the sha512 package-lock.json pins; no install scripts run.
set -euo pipefail

die() { echo "sign-update: $*" >&2; exit 1; }

[[ $# -eq 2 ]] || die "usage: sign-update.sh <archive> <version>"
archive="$1" version="$2"
[[ -f "$archive" ]] || die "no archive at $archive"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "version must be X.Y.Z, not '$version'"
[[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]] || die "TAURI_SIGNING_PRIVATE_KEY isn't set"
lock="${SIGN_UPDATE_LOCKFILE:-$(dirname "$0")/../package-lock.json}"
[[ -f "$lock" ]] || die "no lockfile at $lock"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) native=cli-linux-x64-gnu ;;
  Linux-aarch64) native=cli-linux-arm64-gnu ;;
  Darwin-arm64) native=cli-darwin-arm64 ;;
  Darwin-x86_64) native=cli-darwin-x64 ;;
  *) die "no Tauri signer for $(uname -s)-$(uname -m)" ;;
esac

locked() {
  node -e '
    const p = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).packages["node_modules/" + process.argv[2]];
    if (!p || !p.version || !p.resolved || !/^sha512-/.test(p.integrity || "")) process.exit(1);
    console.log(p.version, p.resolved, p.integrity);
  ' "$lock" "$1" || die "package-lock.json has no pinned $1"
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fetch() {
  local pkg="$1" want="$2" got url integrity tgz
  tgz="$work/$(basename "$pkg").tgz"
  read -r got url integrity < <(locked "$pkg")
  [[ -z "$want" || "$got" == "$want" ]] || die "$pkg is $got in the lockfile, not $want"
  [[ "$url" == "https://registry.npmjs.org/$pkg/-/$(basename "$pkg")-$got.tgz" ]] || die "$pkg resolves to an unexpected URL: $url"
  curl -fsSL --proto '=https' --tlsv1.2 -o "$tgz" "$url" || die "can't download $url"
  [[ "sha512-$(openssl dgst -sha512 -binary "$tgz" | openssl base64 -A)" == "$integrity" ]] \
    || die "$pkg $got doesn't match the sha512 in package-lock.json"
  mkdir -p "$work/node_modules/$pkg"
  tar -xzf "$tgz" -C "$work/node_modules/$pkg" --strip-components 1
  echo "$got"
}

cli_version=$(fetch @tauri-apps/cli "")
fetch "@tauri-apps/$native" "$cli_version" > /dev/null

# The signer runs with the key in its environment: clear anything that could load code into it.
env -u NAPI_RS_NATIVE_LIBRARY_PATH -u NODE_OPTIONS -u NODE_PATH \
  node "$work/node_modules/@tauri-apps/cli/tauri.js" signer sign --app-version "$version" "$archive" > /dev/null
[[ -s "$archive.sig" ]] || die "the signer didn't write $archive.sig"
echo "Signed $archive as version $version with @tauri-apps/cli $cli_version"
