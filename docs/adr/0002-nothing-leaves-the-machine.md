# Nothing leaves the machine except GitHub traffic through the GitHub CLI

CopilotBar must be safe to install on any Mac, including managed ones on Copilot Business/Enterprise plans. It reads local Copilot logs read-only and talks only to GitHub, for the user's plan data (Entitlement, reset date and AIC used) and, since ADR-0005, for its own releases. It sends no telemetry, crash reports or analytics, and no log content or usage data goes to any other host. Any feature that needs another network destination is out of scope unless this decision is revisited.

**Amended by [ADR-0005](0005-github-cli-for-install-update-and-plan-data.md).** CopilotBar no longer borrows sign-ins, saves tokens in the macOS Keychain or makes network calls of its own. All GitHub traffic goes through the GitHub CLI (`gh`), and CopilotBar never sees a token. ADR-0005 lists the network destinations.
