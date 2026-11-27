#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$ROOT/scripts/sign-update.sh"
FAILED=0
PASSED=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

pass() { echo "ok   - $1"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL - $1"; shift; for line in "$@"; do echo "       $line"; done; FAILED=$((FAILED + 1)); }

CLI_VERSION=$(node -p 'require(process.argv[1]).packages["node_modules/@tauri-apps/cli"].version' "$ROOT/package-lock.json")
(cd "$WORK" && npx --yes --package="@tauri-apps/cli@$CLI_VERSION" tauri signer generate --ci -p test-password -w "$WORK/key" > /dev/null 2>&1) \
  || { echo "can't generate a test key"; exit 1; }
KEY=$(cat "$WORK/key")
PUBKEY=$(cat "$WORK/key.pub")

verify() {
  node -e '
    const fs = require("fs"), crypto = require("crypto");
    const [archive, pub] = process.argv.slice(1);
    const text = (b64) => Buffer.from(b64.trim(), "base64").toString("utf8").split("\n");
    const pk = Buffer.from(text(pub)[1], "base64");
    const [, sigLine, trusted, globalLine] = text(fs.readFileSync(archive + ".sig", "utf8"));
    const sig = Buffer.from(sigLine, "base64"), global = Buffer.from(globalLine, "base64");
    const key = crypto.createPublicKey({ key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), pk.subarray(10)]), format: "der", type: "spki" });
    if (sig.subarray(0, 2).toString() !== "ED") throw "not a prehashed signature";
    if (!sig.subarray(2, 10).equals(pk.subarray(2, 10))) throw "different key id";
    const hash = crypto.createHash("blake2b512").update(fs.readFileSync(archive)).digest();
    if (!crypto.verify(null, hash, key, sig.subarray(10))) throw "signature does not match the archive";
    const comment = trusted.replace(/^trusted comment: /, "");
    if (!crypto.verify(null, Buffer.concat([sig.subarray(10), Buffer.from(comment)]), key, global)) throw "trusted comment not signed";
    console.log(comment);
  ' "$1" "$2"
}

new_archive() {
  local dir
  dir=$(mktemp -d "$WORK/case.XXXXXX")
  head -c 4096 /dev/urandom > "$dir/CopilotBar.app.tar.gz"
  echo "$dir/CopilotBar.app.tar.gz"
}

sign() { TAURI_SIGNING_PRIVATE_KEY="$KEY" TAURI_SIGNING_PRIVATE_KEY_PASSWORD=test-password "$SCRIPT" "$@"; }

archive=$(new_archive)
if out=$(sign "$archive" 1.4.2 2>&1) && comment=$(verify "$archive" "$PUBKEY" 2>&1) \
  && [[ "$comment" == timestamp:*$'\t'file:CopilotBar.app.tar.gz$'\t'version:1.4.2 ]]; then
  pass "signs the archive, bound to the release version"
else
  fail "signs the archive, bound to the release version" "$out" "${comment:-}"
fi

printf 'x' >> "$archive"
if verify "$archive" "$PUBKEY" > /dev/null 2>&1; then
  fail "the signature covers the whole archive"
else
  pass "the signature covers the whole archive"
fi

archive=$(new_archive)
node -e '
  const fs = require("fs"), [src, dst] = process.argv.slice(1), lock = JSON.parse(fs.readFileSync(src, "utf8"));
  lock.packages["node_modules/@tauri-apps/cli"].integrity = "sha512-" + Buffer.alloc(64).toString("base64");
  fs.writeFileSync(dst, JSON.stringify(lock));
' "$ROOT/package-lock.json" "$WORK/tampered-lock.json"
if out=$(SIGN_UPDATE_LOCKFILE="$WORK/tampered-lock.json" sign "$archive" 1.4.2 2>&1); then
  fail "refuses a signer that doesn't match the lockfile" "$out"
elif [[ -e "$archive.sig" || "$out" != *"doesn't match the sha512"* ]]; then
  fail "refuses a signer that doesn't match the lockfile" "$out"
else
  pass "refuses a signer that doesn't match the lockfile"
fi

archive=$(new_archive)
if out=$(TAURI_SIGNING_PRIVATE_KEY="" "$SCRIPT" "$archive" 1.4.2 2>&1) || [[ -e "$archive.sig" ]]; then
  fail "refuses to run without a signing key" "$out"
else
  pass "refuses to run without a signing key"
fi

archive=$(new_archive)
if out=$(sign "$archive" v1.4.2 2>&1) || [[ -e "$archive.sig" ]]; then
  fail "refuses a version that isn't X.Y.Z" "$out"
else
  pass "refuses a version that isn't X.Y.Z"
fi

echo "$PASSED passed, $FAILED failed"
[[ $FAILED -eq 0 ]]
