# Local-first usage, with GitHub's plan data only for the Allowance

CopilotBar has to work the same way on personal plans and on Copilot Business/Enterprise. GitHub's billing API reports per-model AI Credits only for plans you pay for yourself, so it gives nothing on a managed machine (or on a Free plan). We therefore get every breakdown (by model, day, hour, Session, Project and Client) from the local Copilot logs. `copilot_internal/user` is used only for what nothing else provides: the Allowance, the reset date, the plan, and GitHub's own counter of AIC used in the Cycle. Used is the larger of the logs and that counter. Anything GitHub counted that the logs don't show is shown as Unlogged Usage, without splitting it by model. The billing API is not used, and there are no dollar estimates, because Copilot plans are sold as bundles.

## Consequences

- Usage that leaves no local trace adds to Unlogged Usage but has no model, token or Session detail. That includes the cloud coding agent, github.com, mobile, other machines, and VS Code's Claude and Codex harnesses.
- If GitHub later offers per-user usage data on every plan, this decision is worth revisiting.
