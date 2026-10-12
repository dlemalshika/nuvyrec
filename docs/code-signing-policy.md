# Code signing policy

## Current status

SignPath Foundation approval has **not** been obtained. Existing Nuvyrec releases
are unsigned. The repository contains a proposed SignPath integration; that is
not a claim that SignPath provides a certificate or endorses this project.

If approved, add the Foundation-required attribution here and on download pages:
“Free code signing provided by [SignPath.io](https://signpath.io), certificate by
[SignPath Foundation](https://signpath.org).” Do not describe this as active before
approval and before verifying the actual release signatures.

## Proposed roles

- Author/committer and reviewer: [dlemalshika](https://github.com/dlemalshika).
- Signing approver: [dlemalshika](https://github.com/dlemalshika).

The repository owner must confirm these roles during onboarding, enable MFA for
GitHub and SignPath, and review contributor changes before release signing.
SignPath signing policies must require manual approval of each signing request.
Use a protected `code-signing` GitHub environment and restrict signing to this
repository's protected main branch. Do not enable secrets for fork/PR execution.

## Signing scope

Sign only Nuvyrec.exe, recorder_core.dll and the Nuvyrec installer built from this
repository on GitHub-hosted Windows runners. The artifact configurations restrict
product names and versions. Flutter, FFmpeg and Visual C++ runtime binaries are
upstream dependencies and must not be signed using this project's subscription.

Returned files must have valid Authenticode signatures, a timestamp certificate,
the configured expected signer thumbprint and matching product metadata before
they enter the signed release output. Keep the signing identity consistent across
versions, and recompute checksums after signing. Never replace already published
unsigned assets silently; prepare a new version for a future signed release.

## Privacy

See [Privacy policy](privacy.md). No recordings, camera/audio data or stream keys
are uploaded to SignPath by the release workflow. Only built application binaries
and build-origin metadata are submitted for signing.

## References

- [Foundation conditions](https://signpath.org/terms)
- [GitHub origin verification](https://docs.signpath.io/trusted-build-systems/github)
- [Windows SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
