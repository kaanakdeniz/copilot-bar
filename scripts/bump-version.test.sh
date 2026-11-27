#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$ROOT/scripts/bump-version.sh"
FAILED=0
PASSED=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

new_repo() {
  local dir
  dir=$(mktemp -d "$WORK/repo.XXXXXX")
  mkdir -p "$dir/src-tauri"
  cp "$ROOT/package.json" "$ROOT/package-lock.json" "$dir/"
  cp "$ROOT/src-tauri/Cargo.toml" "$ROOT/src-tauri/Cargo.lock" "$dir/src-tauri/"
  (
    cd "$dir" || exit 1
    set_version 0.2.0
    git init -q -b main
    git config user.name "Test"
    git config user.email "test@example.com"
    git config commit.gpgsign false
    git config tag.gpgsign false
    git add -A
    git commit -q -m "Initial"
  )
  echo "$dir"
}

set_version() {
  node -e '
    const fs = require("fs"), v = process.argv[1];
    const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
    pkg.version = v;
    fs.writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");
    const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
    lock.version = v; lock.packages[""].version = v;
    fs.writeFileSync("package-lock.json", JSON.stringify(lock, null, 2) + "\n");
  ' "$1"
  perl -0pi -e "s/(\\[package\\]\\nname = \"copilot-bar\"\\nversion = )\"[^\"]*\"/\${1}\"$1\"/" src-tauri/Cargo.toml
  perl -0pi -e "s/(name = \"copilot-bar\"\\nversion = )\"[^\"]*\"/\${1}\"$1\"/" src-tauri/Cargo.lock
}

versions() {
  node -p 'require("./package.json").version'
  node -p 'require("./package-lock.json").version'
  node -p 'require("./package-lock.json").packages[""].version'
  awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version = /{gsub(/"/,"",$3); print $3; exit}' src-tauri/Cargo.toml
  awk '/^name = "copilot-bar"$/{getline; gsub(/"/,"",$3); print $3; exit}' src-tauri/Cargo.lock
}

pass() { echo "ok   - $1"; }
fail() { echo "FAIL - $1"; shift; for line in "$@"; do echo "       $line"; done; }

run() { OUT=$("$SCRIPT" "$@" 2>&1); STATUS=$?; }

test_bump_updates_every_file_commits_and_tags() {
  local name="bump to a valid version updates all four files, commits once and tags"
  cd "$(new_repo)" || return
  local before
  before=$(git rev-list --count HEAD)
  run 0.3.0
  local got expected
  got=$(versions | tr '\n' ' ')
  expected="0.3.0 0.3.0 0.3.0 0.3.0 0.3.0 "
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  if [[ "$got" != "$expected" ]]; then fail "$name" "versions: $got" "expected: $expected"; return; fi
  if [[ $(git rev-list --count HEAD) -ne $((before + 1)) ]]; then fail "$name" "expected exactly one new commit"; return; fi
  if [[ "$(git log -1 --format=%s)" != "Release v0.3.0" ]]; then fail "$name" "subject: $(git log -1 --format=%s)"; return; fi
  if [[ "$(git rev-parse 'v0.3.0^{commit}' 2>/dev/null)" != "$(git rev-parse HEAD)" ]]; then fail "$name" "tag v0.3.0 not on HEAD"; return; fi
  if [[ -n "$(git status --porcelain)" ]]; then fail "$name" "working tree left dirty" "$(git status --porcelain)"; return; fi
  if [[ "$(git show --name-only --format= HEAD | sort | tr '\n' ' ')" != "package-lock.json package.json src-tauri/Cargo.lock src-tauri/Cargo.toml " ]]; then
    fail "$name" "commit touched: $(git show --name-only --format= HEAD | tr '\n' ' ')"; return
  fi
  pass "$name"
}

expect_refusal() {
  local name="$1" pattern="$2"; shift 2
  local head status_before
  head=$(git rev-parse HEAD)
  status_before=$(git status --porcelain)
  run "$@"
  if [[ $STATUS -eq 0 ]]; then fail "$name" "expected a non-zero exit" "$OUT"; return; fi
  if ! grep -qiE "$pattern" <<<"$OUT"; then fail "$name" "message didn't match /$pattern/:" "$OUT"; return; fi
  if [[ "$(git rev-parse HEAD)" != "$head" ]]; then fail "$name" "a commit was made"; return; fi
  if [[ -n "$(git tag)" ]]; then fail "$name" "a tag was made: $(git tag)"; return; fi
  if [[ "$(git status --porcelain)" != "$status_before" ]]; then fail "$name" "working tree changed"; return; fi
  if [[ "$(versions | sort -u)" != "0.2.0" ]]; then fail "$name" "versions changed: $(versions | tr '\n' ' ')"; return; fi
  pass "$name"
}

test_refuses_dirty_working_tree() {
  cd "$(new_repo)" || return
  echo "stray" > notes.txt
  expect_refusal "refuses a dirty working tree" "uncommitted|dirty" 0.3.0
}

test_refuses_invalid_versions() {
  cd "$(new_repo)" || return
  local v
  for v in "" 1.2 1.2.3.4 v0.3.0 0.3.0-beta 01.2.3 a.b.c "0.3.0 "; do
    expect_refusal "refuses invalid version '$v'" "invalid|usage" "$v"
  done
}

test_refuses_versions_not_greater_than_current() {
  cd "$(new_repo)" || return
  local v
  for v in 0.2.0 0.1.9 0.1.10 0.0.99; do
    expect_refusal "refuses $v when the current version is 0.2.0" "greater" "$v"
  done
}

test_compares_versions_numerically() {
  local name="compares each part numerically (0.10.0 is greater than 0.9.0)"
  cd "$(new_repo)" || return
  set_version 0.9.0 && git commit -qam "At 0.9.0"
  run 0.10.0
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  pass "$name"
}

test_check_passes_when_tag_matches_every_file() {
  local name="check passes when the tag matches every file"
  cd "$(new_repo)" || return
  run --check v0.2.0
  if [[ $STATUS -ne 0 ]]; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  pass "$name"
}

check_fails_on() {
  local name="check fails when $1 differs" file="$1"
  cd "$(new_repo)" || return
  perl -0pi -e "$2" "$file"
  git commit -qam "Drift $file"
  run --check v0.2.0
  if [[ $STATUS -eq 0 ]]; then fail "$name" "expected a non-zero exit" "$OUT"; return; fi
  if ! grep -F "$file" <<<"$OUT" | grep -F 0.2.0 | grep -qF 0.2.1; then
    fail "$name" "message should name $file, the tag version and the file's version:" "$OUT"; return
  fi
  pass "$name"
}

test_check_fails_when_package_json_differs() {
  check_fails_on package.json 's/"version": "0.2.0"/"version": "0.2.1"/'
}
test_check_fails_when_package_lock_differs() {
  # Only the root package entry, not the top-level field.
  check_fails_on package-lock.json 's/("": \{\n\s*"name": "copilot-bar",\n\s*"version": )"0.2.0"/$1"0.2.1"/'
}
test_check_fails_when_cargo_toml_differs() {
  check_fails_on src-tauri/Cargo.toml 's/^version = "0.2.0"/version = "0.2.1"/m'
}
test_check_fails_when_cargo_lock_differs() {
  check_fails_on src-tauri/Cargo.lock 's/(name = "copilot-bar"\nversion = )"0.2.0"/$1"0.2.1"/'
}

test_check_fails_when_tag_differs_from_files() {
  local name="check fails when the tag doesn't match the files"
  cd "$(new_repo)" || return
  run --check v0.3.0
  if [[ $STATUS -eq 0 ]]; then fail "$name" "expected a non-zero exit" "$OUT"; return; fi
  if [[ $(grep -c 0.3.0 <<<"$OUT") -lt 4 ]]; then fail "$name" "expected each of the four files reported:" "$OUT"; return; fi
  pass "$name"
}

test_check_refuses_malformed_tag() {
  local name="check refuses a tag that isn't vX.Y.Z"
  cd "$(new_repo)" || return
  run --check 0.2.0
  if [[ $STATUS -eq 0 ]] || ! grep -qi "invalid" <<<"$OUT"; then fail "$name" "exit $STATUS" "$OUT"; return; fi
  pass "$name"
}

for t in $(declare -F | awk '{print $3}' | grep '^test_'); do
  ( "$t" ) | tee "$WORK/out"
  grep -q '^FAIL' "$WORK/out" && FAILED=$((FAILED + 1)) || PASSED=$((PASSED + 1))
done

echo "$PASSED passed, $FAILED failed"
[[ $FAILED -eq 0 ]]
