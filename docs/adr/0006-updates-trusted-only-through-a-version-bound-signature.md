# Updates are trusted only through a signature bound to their version

An Update is installed only if it is signed with the update key built into CopilotBar, and the signature is for exactly that Release's version. Otherwise nothing changes: the check fails closed. A Release isn't trusted on its own, because anyone who can edit a Release on GitHub can edit what is attached to it. The signing key exists only in the signing step of the release workflow; the steps that build and publish a Release never have it.

## Considered options

- **Trusting SHA256SUMS.** Whoever can change a Release can change its checksums too, so the checksums prove nothing about where an archive came from.
- **GitHub artifact attestations as the trust root.** They can't be verified offline inside the app, and they don't work for private repositories on personal accounts.
- **A signature not bound to the version.** An older, validly signed archive could be re-uploaded as a newer Release and downgrade users.

## Consequences

- Losing the update key means existing installs can't update and must be reinstalled by hand.
- The attestation and SHA256SUMS on each Release are for people checking by hand; the app doesn't rely on them.
- Changing the update key requires a manual reinstall.
