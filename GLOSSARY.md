# CopilotBar

A macOS menu bar app that shows one person's GitHub Copilot usage: how many AI Credits they have spent in the current Cycle, and whether that spending keeps them within their Allowance.

## Credits and limits

**AI Credit (AIC)**:
The unit GitHub bills Copilot usage in. Every figure about spending is in AIC.
_Avoid_: credit, premium request, AIU

**Allowance**:
The number of AIC the user expects to spend in one Cycle. It is GitHub's Entitlement unless the user sets their own number. When GitHub does not permit Overage, the Allowance cannot exceed the Entitlement, because Copilot stops there.
_Avoid_: budget (on its own), monthly budget, quota, limit

**Entitlement**:
The monthly AIC amount GitHub reports for the user's plan. It is the default for the Allowance.
_Avoid_: quota

**Logged AIC**:
The AIC recorded in the Copilot logs on this Mac. Every breakdown (by model, day, hour, Session, Project or Client) comes from it.
_Avoid_: local credits, tracked usage

**Used**:
The AIC spent so far in this Cycle: whichever is larger, Logged AIC or GitHub's own counter. Pace and the Allowance are measured against it.
_Avoid_: spent, consumed

**Unlogged Usage**:
The part of Used that GitHub counted but the logs on this Mac do not show, such as usage on github.com, mobile, another machine, the cloud coding agent, or VS Code's Claude and Codex harnesses. It appears as its own "Not in local logs" slice, so breakdowns still add up to Used.
_Avoid_: other, untracked, remainder

**Overage**:
AIC spent after the Entitlement is used up, billed separately. It exists only when GitHub permits overage for the user; otherwise Copilot stops working when the Entitlement runs out.
_Avoid_: overspend, extra usage, spend

**Cycle**:
The period one Allowance covers. It ends at GitHub's reset date, and it starts and ends at midnight UTC.
_Avoid_: month, billing month, billing period

## Pace

**Even Pace**:
The amount the user would have spent by now if they spent the Allowance evenly across the Cycle.
_Avoid_: expected, target, ideal

**Pace Status**:
Where the user's spending stands against Even Pace: **Under**, **On Pace** (within the tolerance), **Ahead**, or **Exhausted** (the Allowance is all spent).
_Avoid_: behind (use Under), used up / over (use Exhausted)

**Daily Budget**:
How much AIC the user can spend today and still stay within the Allowance: what was left at the start of today, divided evenly over today and the remaining days of the Cycle.
_Avoid_: today's share, can spend per day, daily allowance

## Usage

**Client**:
The tool the user worked in when they used Copilot: Copilot CLI, VS Code, or the Copilot App.
_Avoid_: source, surface, tool, app

**Session**:
One conversation with Copilot in a single Client, such as one Copilot CLI run or one VS Code chat.
_Avoid_: chat, conversation, run

**Project**:
The working directory the user was in while using Copilot. A Project can hold many Sessions. A Session with no real working directory, such as a Copilot App chat, has no Project.
_Avoid_: repo, workspace, context

## Alerts

**Burst**:
AIC spent within any one hour, across all Clients and Sessions, that reaches the user's Burst threshold.
_Avoid_: spike, runaway

**Runaway Session**:
A single Session that reaches the Burst threshold on its own within one hour.
_Avoid_: burst, expensive session

## GitHub

**Sign-in**:
The GitHub account the GitHub CLI is signed in with. CopilotBar gets plan data and Updates through it and has no sign-in of its own. If it doesn't work, it says why and what the user does next: install the GitHub CLI, sign in, authorise it for the organisation's SSO, or switch to the same account as the Copilot CLI.
_Avoid_: login, token, auth, account (on its own)

**Release**:
A published version of CopilotBar on GitHub: the app, its update signature, a DMG and their checksums. Drafts and pre-releases are not Releases.
_Avoid_: build, tag, version (for the published thing)

**Update**:
A Release newer than the CopilotBar that is running. It is installed only when the user asks, and only if it is signed with CopilotBar's update key for that Release's version; otherwise nothing changes.
_Avoid_: upgrade, patch, auto-update
