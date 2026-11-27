# Developing CopilotBar


```sh
npm ci
npm run tauri dev          # app with hot reload
npm run dev                # UI only, in a browser, on demo data:
                           #   http://localhost:1420/?view=popover&demo=ahead&mode=calendar&theme=dark
                           #   http://localhost:1420/?view=dashboard&demo=onpace&tab=models
npm test                   # TypeScript unit tests (vitest)
npx tsc --noEmit           # type check
cd src-tauri && cargo test # Rust unit tests
./scripts/release.sh       # DMG; signs and notarizes when APPLE_SIGNING_IDENTITY/APPLE_ID/… are set
```

CI (`.github/workflows/ci.yml`) runs `cargo test --locked` on macOS, `cargo deny check` (advisories, licenses, sources and bans), `npx tsc --noEmit`, `npx vitest run` and `npm audit` for the UI, and the tests of the version bump and release packaging scripts (`scripts/*.test.sh`). The release workflow runs the same jobs before it builds.

**Troubleshooting:**
- The log is at `~/Library/Application Support/dev.copilotbar/copilotbar.log`.
- `/Applications/CopilotBar.app/Contents/MacOS/copilot-bar --dump-snapshot out.json [--offline]` writes exactly what the UI sees. `--offline` skips GitHub.
- `gh auth status` and `gh api copilot_internal/user` show what CopilotBar gets from `gh`.

**Layout:**
- `src-tauri/src/core`: log parsers, the `gh` runner and plan data, pace and settings, with no UI code.
- `src-tauri/src`: tray, windows and notifications.
- `src`: the React UI.
- `src/dev`: browser-only demo data and design tools (`?view=studio`, `?view=icons`), loaded only by `npm run dev` and left out of the app.

## Releasing

See [RELEASING.md](RELEASING.md).
