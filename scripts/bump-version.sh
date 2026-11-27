#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

die() { echo "bump-version: $*" >&2; exit 1; }

write_version() {
  node -e '
    const fs = require("fs"), v = process.argv[1];
    const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
    pkg.version = v;
    fs.writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");
    const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
    lock.version = v;
    lock.packages[""].version = v;
    fs.writeFileSync("package-lock.json", JSON.stringify(lock, null, 2) + "\n");
  ' "$1"
  awk -v v="$1" '
    /^\[package\]$/ { p = 1 } /^\[/ && !/^\[package\]$/ { p = 0 }
    p && /^version = / { print "version = \"" v "\""; p = 0; next } { print }
  ' src-tauri/Cargo.toml > src-tauri/Cargo.toml.tmp && mv src-tauri/Cargo.toml.tmp src-tauri/Cargo.toml
  awk -v v="$1" '
    after { print "version = \"" v "\""; after = 0; next }
    /^name = "copilot-bar"$/ { after = 1 } { print }
  ' src-tauri/Cargo.lock > src-tauri/Cargo.lock.tmp && mv src-tauri/Cargo.lock.tmp src-tauri/Cargo.lock
}

greater() {
  local a b i
  IFS=. read -r -a a <<<"$1"
  IFS=. read -r -a b <<<"$2"
  for i in 0 1 2; do
    (( 10#${a[i]} > 10#${b[i]} )) && return 0
    (( 10#${a[i]} < 10#${b[i]} )) && return 1
  done
  return 1
}

read_versions() {
  node -e '
    const pkg = require("./package.json"), lock = require("./package-lock.json");
    console.log("package.json " + pkg.version);
    console.log("package-lock.json " + lock.version);
    console.log("package-lock.json " + (lock.packages?.[""]?.version ?? "(missing)"));
  '
  echo "src-tauri/Cargo.toml $(awk '
    /^\[package\]$/ { p = 1; next } /^\[/ { p = 0 }
    p && /^version = / { gsub(/"/, "", $3); print $3; exit }
  ' src-tauri/Cargo.toml)"
  echo "src-tauri/Cargo.lock $(awk '
    /^name = "copilot-bar"$/ { getline; gsub(/"/, "", $3); print $3; exit }
  ' src-tauri/Cargo.lock)"
}

FILES=(package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock)

SEMVER='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
usage() { die "usage: bump-version.sh X.Y.Z | bump-version.sh --check vX.Y.Z"; }

if [[ "${1:-}" == "--check" ]]; then
  [[ $# -eq 2 ]] || usage
  tag="$2"
  [[ "$tag" == v* && "${tag#v}" =~ $SEMVER ]] || die "invalid tag '$tag'; expected vX.Y.Z, e.g. v0.3.0"
  want="${tag#v}"
  bad=0
  while read -r file got; do
    if [[ "$got" != "$want" ]]; then
      echo "bump-version: tag $tag wants $want but $file has ${got:-(missing)}" >&2
      bad=1
    fi
  done < <(read_versions)
  [[ $bad -eq 0 ]] || die "version mismatch: the tag must equal the version in every file (set it with scripts/bump-version.sh)"
  echo "Tag $tag matches the version in every file."
  exit 0
fi

[[ $# -eq 1 ]] || usage
new="$1"
[[ "$new" =~ $SEMVER ]] || die "invalid version '$new'; expected X.Y.Z, e.g. 0.3.0"
[[ -z "$(git status --porcelain)" ]] || die "the working tree has uncommitted changes; commit or stash them first"
current=$(node -p 'require("./package.json").version')
greater "$new" "$current" || die "$new is not greater than the current version $current"
write_version "$new"
git add -- "${FILES[@]}"
git commit -q -m "Release v$new" -- "${FILES[@]}"
git tag -a "v$new" -m "Release v$new"
echo "Tagged v$new. Publish with: git push --follow-tags"
