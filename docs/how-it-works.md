# How CopilotBar works

The full reference behind the [README](../README.md). Terms are defined in [GLOSSARY.md](../GLOSSARY.md).

## Signing in with `gh`

CopilotBar never asks for, sees or stores a GitHub token, and never touches the Keychain ([ADR-0005](adr/0005-github-cli-for-install-update-and-plan-data.md)). Your plan data comes from `gh api copilot_internal/user`, through your own `gh` sign-in. Without `gh`, local usage still works; you just don't get GitHub's Entitlement, reset date or counter.

1. Install `gh`: `brew install gh`. CopilotBar runs `gh` only from `/opt/homebrew/bin`, `/usr/local/bin` or `/usr/bin`, never from your PATH.
2. Sign in: `gh auth login`, and choose **GitHub.com**.
3. If your organisation uses SAML single sign-on, authorise `gh` for it: run `gh auth refresh`, and on the browser page click **Authorize** next to your organisation. You can also do it at **GitHub → Settings → Applications → Authorized OAuth Apps → GitHub CLI**.
4. Use the same account as the Copilot CLI. If `gh` is signed in to several accounts, CopilotBar warns when its active one differs from the Copilot CLI's, because the plan shown would be someone else's. Fix it with `gh auth switch --user <your-copilot-login>`.

Settings → **Sign-in** shows the current state and its next step: signed in as you, `gh` not installed, signed out, SSO not authorised, a different account from the Copilot CLI's, or another error. `gh auth status` shows what `gh` itself sees.

## Using it

**Left-click** the menu bar item to open the popover. **Right-click** it for Open Dashboard, Refresh Now, Settings, Check for Updates and Quit. Press **⌥⌘C** from anywhere to open the popover (you can change or turn off the shortcut).

The popover shows the Cycle against your Allowance, your Pace Status and your Daily Budget. In the popover:

| Key | Does |
|---|---|
| **⌘I** | Insights: the daily chart and models |
| **⌘K** | Calendar: each day of the Cycle, shaded by AIC; click a day for its split by model |
| **⌘R** | Refresh |
| **⌘D** | Open the dashboard |
| **⌘,** | Settings |
| **Esc** | Back, or close |

The **dashboard** has four tabs. Every tab can be filtered by Client and date range, and **Export CSV** writes every logged call in the selection to `~/Downloads`.
- **Overview:** Used against Even Pace and the projection, AIC or tokens per day, your Daily Budget and averages.
- **Models:** each model's share of AIC and tokens.
- **Projects:** AIC per Project, and every Session with its Client, models and start time. Runaway Sessions are flagged.
- **Settings.**

**Settings** cover:
- **Menu bar:** icon, number or both; which icon; and percent used, AIC used, AIC left, or Used / Allowance. Optionally green when you're Under or On Pace, orange when you're Ahead and red when the Allowance is Exhausted (no colour when no Allowance is set).
- **Popover:** a gauge or a bar for the Cycle, and whether the big number is Used or remaining.
- **Allowance:** your own number of AIC per Cycle, or empty to use GitHub's Entitlement. If your plan doesn't permit Overage, the Allowance can't exceed the Entitlement.
- **Notifications,** each sent once per Cycle: when Used reaches a percent of the Allowance (default 75%, 90% and 100%), when you're ahead of Even Pace, and a **Burst alert** (default 10 AIC within one hour, 0 turns it off).
- **Sign-in:** the GitHub CLI's sign-in and, if something is wrong, the command that fixes it.
- **Updates:** the version you're running, **Check for Updates** and the last result, and **Automatically check for updates** (on by default).
- **General:** launch at login, the keyboard shortcut, theme (system, light or dark) and text size.
- **Advanced:** the "on pace" tolerance, whether the week starts on Monday, a colour-blind-safe chart palette, how often to check GitHub (default every 5 minutes; local logs are re-read every minute and whenever you open the popover), and the Copilot session logs folder.

## Updates

CopilotBar checks for a newer version through `gh` (`gh api repos/<owner>/<name>/releases`) at launch, then every 24 hours, and whenever you pick **Check for Updates…** in the menu bar menu or in Settings → **Updates**. Only published releases count, not drafts or pre-releases. When a newer one exists, you get a notification and the menu gains **Update to x.y.z** (also in Settings → **Updates**). Clicking it downloads `CopilotBar.app.tar.gz` and its signature through `gh release download`, checks the signature against the update key built into the app (it must be signed by that key, for that version), replaces the app where it is and relaunches. If that folder isn't writable, the update goes to `~/Applications` instead and a notification says so. A missing or invalid signature, a failed download or an unpackable archive installs nothing and leaves your app as it was; Settings then says why and links to the release page. Nothing `gh` downloads is quarantined, so no Gatekeeper prompt appears. Turning off **Automatically check for updates** stops the launch and daily checks; **Check for Updates** still works. If a check fails, Settings says why and links to the release page.

## Where the numbers come from

CopilotBar is local-first ([ADR-0003](adr/0003-local-first-no-billing-api.md)). It has two sources:

| Source | Gives |
|---|---|
| Copilot session logs: `~/.copilot/session-state/<id>/events.jsonl` and `workspace.yaml` | Every breakdown: per call, the model, tokens, AIC (`copilotUsage.totalNanoAiu`, where 1 AIC = 10⁹ nano-AIU), Session, Project and Client. Older Copilot CLI versions log per-Session totals instead, and those are read too. |
| GitHub, through `gh api copilot_internal/user` | Your plan, your Entitlement, the reset date, whether Overage is permitted, and GitHub's own counter of AIC used this Cycle. |

**Clients.** These all write to `~/.copilot/session-state`. The Client comes from `client_name` in each Session's `workspace.yaml`:

| Client | `client_name` |
|---|---|
| Copilot CLI | `github/cli`, `sdk` (and any name CopilotBar doesn't know, so no usage is dropped) |
| VS Code, Copilot harness | `vscode-agent-host` |
| Copilot App | `github/autopilot` (its Sessions run in a scratch folder, so they have no Project) |

**Used** is the larger of Logged AIC and GitHub's counter. Whatever GitHub counted that the logs don't show is **Unlogged Usage**, shown as its own "Not in local logs" slice so breakdowns still add up to Used. It has no model, token or Session detail. It includes usage on github.com, mobile, other machines, the cloud coding agent, and VS Code's Local, Claude and Codex harnesses, which leave no usage in local logs.

**Overage** is AIC used after the Entitlement runs out. It appears only if GitHub permits Overage for your plan, and it is billed separately. Otherwise Copilot stops at the Entitlement until the reset.

**Cycles and days** ([ADR-0004](adr/0004-utc-cycles-local-days.md)). A Cycle starts and ends at GitHub's reset time, midnight UTC. "Today", daily charts and the calendar use your local midnight. So the first and last local day of a Cycle can be partial days.

**Pace.** *Even Pace* is the Allowance × the share of the Cycle elapsed. Your *Daily Budget* is what was left at the start of today, divided evenly over today and the remaining days of the Cycle.

**Privacy** ([ADR-0002](adr/0002-nothing-leaves-the-machine.md), [ADR-0005](adr/0005-github-cli-for-install-update-and-plan-data.md)). CopilotBar reads your Copilot logs read-only. It has no network code of its own: its only GitHub traffic is the `gh` commands it runs, which reach `api.github.com` for your plan data and the update check, and GitHub's release-asset host when you install an update. It sends no telemetry, crash reports or analytics, and no log content or usage data goes anywhere else. It never sees, stores or asks for a token, and never touches the Keychain.
