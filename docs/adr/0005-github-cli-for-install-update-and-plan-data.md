# Install, update and plan data through the GitHub CLI

CopilotBar has to work for accounts in organisations that enforce SAML SSO, including GitHub Enterprise Cloud. Borrowing tokens from the Keychain (the Copilot CLI's item, `gh auth token`) or asking for a pasted PAT brought Keychain prompts, re-prompted after every ad-hoc-signed build, and left CopilotBar holding a GitHub token and making its own HTTPS calls. A browser-downloaded DMG is also quarantined, and MDM may hide **Open Anyway**. So the GitHub CLI (`gh`) is CopilotBar's only GitHub client and its only identity, for installing, for updates and for plan data. Developers in those organisations already have `gh`, and it already carries their SSO-authorised sign-in.

- **No token in CopilotBar.** CopilotBar never sees, stores or asks for a token. It has no Keychain access, no PAT field, no `COPILOTBAR_TOKEN` variable and no token borrowing.
- **No network code in CopilotBar.** It has no HTTP client. Plan data comes from `gh api copilot_internal/user`. Update checks use `gh api` on the repository's releases, and downloads use `gh release download`. All GitHub traffic is `gh`'s.
- **Network destinations,** all reached by `gh`: `api.github.com` (plan data, releases), `github.com`, and GitHub's release-asset host (`objects.githubusercontent.com` and `release-assets.githubusercontent.com`) for downloads. CopilotBar passes `--hostname github.com`, so a `GH_HOST` setting can't send its calls elsewhere, and it turns off `gh`'s own update notifier.
- **Only fixed install locations.** `gh` is run from `/opt/homebrew/bin/gh` (Homebrew on Apple Silicon), `/usr/local/bin/gh` (Homebrew on Intel) or `/usr/bin/gh`, never from the PATH, so a planted `gh` earlier on the PATH is never run. It runs with no stdin and prompts disabled, and is given up after a timeout.
- **Sign-in states.** `gh`'s exit status and stderr map to a sign-in state, each with its own next step: signed in, not installed (`brew install gh`), signed out or rejected (`gh auth login`), SSO not authorised (`gh auth refresh`, then **Authorize** for the organisation), `gh`'s active account differs from the Copilot CLI's (`gh auth switch`), and any other error. Local usage works in every state.
- **Install** is a `gh release download` one-liner. Nothing `gh` downloads is quarantined, so Gatekeeper's "Open Anyway" never appears. A DMG stays on each release as a fallback for browser downloads.
- **Updates** are checked through `gh` and installed only when the user clicks. Each update archive is verified against the update public key built into the app before it replaces the installed bundle.

## Considered options

- **Developer ID signing and notarization.** It removes the Gatekeeper prompt for browser downloads, but needs a paid Apple account and its secrets, and still leaves sign-in to solve. Adding it later is only secrets and config.
- **A personal access token.** Users would have to create, paste, rotate and SSO-authorise a token, and CopilotBar would have to store it (the Keychain prompts again) and make its own HTTPS calls.
- **The stock Tauri updater.** It downloads over its own HTTP client, from URLs that need a token for a private or SSO-protected repository. Going through `gh` keeps the one identity and keeps CopilotBar free of network code. Updates are still signed with a minisign-compatible key, as Tauri's updater signs them.

## Consequences

- Without `gh`, CopilotBar shows local usage only, with the Allowance the user sets, and says how to install `gh` and sign in.
- `gh` must be installed in one of the fixed locations. A `gh` installed only somewhere else (for example by a version manager) isn't used.
- Every ad-hoc-signed build has a new code identity. That no longer matters, because CopilotBar uses no Keychain items.
- Losing the update signing key means existing installs can't update and must be reinstalled by hand.
