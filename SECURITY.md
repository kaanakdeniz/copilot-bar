# Security policy

## Supported versions

Only the latest published release gets security fixes. Update with **Check for Updates** in the menu bar menu.

## Reporting a vulnerability

Report it privately through GitHub: **Security → Report a vulnerability** on this repository. Don't open a public issue.

Please include the CopilotBar and macOS versions, what an attacker can do, and the steps to reproduce. Don't include your real usage data, logs with your GitHub login, `--dump-snapshot` output or any token.

You'll get an answer within a week. Fixed issues are credited in the release notes unless you'd rather not be named.

## Scope

CopilotBar's security rests on a few rules. A way to break any of them is in scope:

- **Nothing leaves the machine** ([ADR-0002](docs/adr/0002-nothing-leaves-the-machine.md)). The app has no network code of its own and sends no telemetry. Its only traffic is the `gh` commands it runs.
- **No tokens** ([ADR-0005](docs/adr/0005-github-cli-for-install-update-and-plan-data.md)). The app never asks for, stores or reads a GitHub token, and runs `gh` only from fixed install locations.
- **Signed updates** ([ADR-0006](docs/adr/0006-updates-trusted-only-through-a-version-bound-signature.md)). An update installs only if it is signed by the key built into the app, for exactly that version.
- **Untrusted input.** Copilot session logs, `workspace.yaml` and GitHub's responses are parsed defensively; a crafted file must not crash the app, run code or corrupt the totals.

Out of scope: an attacker who already runs code as your user, GitHub's own services, and the `gh` CLI itself.
