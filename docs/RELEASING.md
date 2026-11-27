# Releasing

## One-time setup

1. Generate the update signing key: `npx tauri signer generate -w ~/.tauri/copilotbar.key`. Put its public key in `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
2. In the repository settings, create an Environment named `release`. Give it required reviewers, and limit its deployment tags to `v*`.
3. Add the private key and its password to that Environment as the secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Keep no repository-level copy.
4. Back up the key and its password in a password manager. Without them, installed apps can't update ([ADR-0006](adr/0006-updates-trusted-only-through-a-version-bound-signature.md)).

A fork that wants its own releases needs its own key and its own `plugins.updater.pubkey`. The release workflow only runs in this repository.

## Cutting a release

```bash
./scripts/bump-version.sh x.y.z
git push --follow-tags
```

`bump-version.sh` sets the version in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`, commits "Release vx.y.z" and tags `vx.y.z`. It refuses a dirty working tree and a version that isn't greater than the current one. `./scripts/bump-version.sh --check vx.y.z` exits non-zero if any of those files disagrees with the tag. The app reads its version from `package.json`.

The tag starts `.github/workflows/release.yml`:

1. **version** checks that the tag matches every version file.
2. **ci** re-runs every CI job.
3. **build** builds one universal app: `CopilotBar.app.tar.gz` and the DMG.
4. **sign** signs the update archive. It is the only job with the signing secrets, it runs in the `release` Environment, and it installs none of the app's dependencies: it runs the Tauri signer at the version and sha512 that `package-lock.json` pins (`scripts/sign-update.sh`).
5. **verify** checks the signature with the app's own verifier and the public key from `tauri.conf.json`, without the secrets, so a secret that doesn't match the key fails the release.
6. **release** opens a **draft** release with the archive, its `.sig`, the DMG, `SHA256SUMS` and a build-provenance attestation.

Check the draft's assets, then publish it. Installed apps see it at their next update check.

## Local builds

`./scripts/release.sh` builds a DMG for this Mac's architecture without tagging. Set `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID` (Developer ID) to have it signed and notarized.
