# SignPath setup for Nuvyrec

Status: workflow and verification scripts prepared; account, Foundation approval,
server-side configuration and credentials are still required. No signed release
has been produced. Signing does not guarantee immediate SmartScreen reputation.

## 1. Apply as the repository owner

Open [SignPath Foundation application](https://signpath.org/apply). Review the
[conditions](https://signpath.org/terms), then supply your own contact details and
accept terms yourself. Suggested project information:

| Field | Value |
|---|---|
| Project | Nuvyrec |
| Source repository | https://github.com/dlemalshika/nuvyrec |
| Download/releases | https://github.com/dlemalshika/nuvyrec/releases |
| Maintainer | dlemalshika |
| Description | Native Windows screen recorder built with Flutter and Rust/FFmpeg; display/window/region capture, webcam overlays and RTMP/RTMPS streaming. |
| Source license | MIT; packaged FFmpeg includes GPL components, with distribution obligations described in docs/release-status.md. |
| Build system | GitHub Actions, GitHub-hosted Windows runners; source build with pinned/checksummed dependencies. |
| Signing policy | docs/code-signing-policy.md |
| Privacy policy | docs/privacy.md |

Do not claim an approved certificate or existing signed release. The Foundation
requires an already released, maintained and documented open-source project with
verifiable reputation. A new project is not guaranteed acceptance. Before applying,
check that a real downloadable release exists and review third-party licensing
and corresponding-source obligations. Our local preparation is not an eligibility
audit or proof of compliance.

## 2. Prepare GitHub and SignPath after approval

Enable MFA on both accounts. Install the SignPath GitHub App only for this repo
when instructed during onboarding. Configure trusted GitHub origin verification
and manual approval for every signing request, restricted to main and hosted
runners. Import these proposed artifact configurations, then validate them in the
SignPath service:

- `packaging/signpath/payload.xml`: Nuvyrec.exe and recorder_core.dll only.
- `packaging/signpath/installer.xml`: the versioned Setup.exe only.

Create a GitHub environment named `code-signing` with the maintainer as required
reviewer and main as the permitted deployment branch. Protect main and review
incoming changes before accepting them. The workflow has no PR trigger and does
not publish GitHub releases automatically.

## 3. Configure credentials without putting tokens in code/chat

GitHub → repository Settings → Environments → code-signing:

| Type | Name | Value |
|---|---|---|
| Secret | SIGNPATH_API_TOKEN | SignPath token with signing-submission permission |
| Variable | SIGNPATH_ORGANIZATION_ID | Organization ID supplied by SignPath |
| Variable | SIGNPATH_PROJECT_SLUG | Your approved project slug |
| Variable | SIGNPATH_POLICY_SLUG | Policy requiring manual approval |
| Variable | SIGNPATH_PAYLOAD_CONFIG | Slug of imported payload config |
| Variable | SIGNPATH_INSTALLER_CONFIG | Slug of imported installer config |
| Variable | SIGNPATH_CERTIFICATE_THUMBPRINT | Expected certificate's 40-digit SHA1 thumbprint, no spaces |

Never paste a token into an issue, commit or assistant message. The thumbprint is
public certificate identity, not a private key. Obtain it through the approved
SignPath certificate/account settings, not from an unverified download.

## 4. Run the workflow

Commit/push the setup files first. Actions → Windows build and SignPath signing →
Run workflow → main. Start with **sign unchecked** to validate the clean hosted
build without credentials. This produces explicitly labelled unsigned artifacts.
The workflow bootstrap is intended only for GitHub-hosted Windows runners.

After approval/configuration, run with **sign checked**. Approve each request in
SignPath (payload first, then installer). The workflow verifies signatures and
timestamps, regenerates the signed bundle manifest and creates checksums for the
final signed installer and portable archive. Download `verified-signed-release`
from the completed workflow. Do not publish the intermediate `unsigned-installer`
artifact. Current integration signs the project app/DLL and outer installer;
third-party runtime files and Inno's generated uninstaller are outside this scope.

Perform clean-Windows installation/recording/uninstall QA, then upload verified
files to a **new** versioned release (for example v1.0.1), with the code signing
policy link in the release notes. Match the version in pubspec before running.
Do not change signed bytes after signing, or reuse v1.0.0 to replace downloads.

## Validation limits

Local script syntax, artifact configuration structure, DLL version metadata and
unsigned-artifact rejection can be checked before onboarding. The actual signing
requests, XML validation by SignPath, clean GitHub-hosted build and SmartScreen
behavior require external accounts/service approval and real end-to-end testing.
