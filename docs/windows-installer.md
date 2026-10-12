# Windows installer

For future signed releases, see [SignPath setup](signpath-setup.md) and the
[Code signing policy](code-signing-policy.md). Account approval is pending.

The per-user Windows installer includes the complete native app, Flutter assets,
Rust engine and FFmpeg runtime. End users need no development tools. The current
build targets x64-compatible Windows 10 1903+ and Windows 11; other platform
installers are not supplied by this pipeline.

## End users

Download `Nuvyrec-Windows-x64-vVERSION-Setup.exe` from the project's GitHub release.
Run it, choose an installation folder and optionally enable the Desktop shortcut.
The default is `%LOCALAPPDATA%\Programs\Nuvyrec`. Open Nuvyrec from the Start Menu.
Windows Settings → Apps can uninstall it. Uninstall retains recordings and the
existing per-user app preferences. Admin rights are not requested by default.

Before upgrading, stop/save recording and exit Nuvyrec, including its tray icon.
Run the newer Setup.exe using the same user account and installation directory.
A stable application ID lets setup recognize that installation. Setup refuses
to proceed when the destination executable is in use; it does not automatically
close the recorder. Uninstall also checks whether the executable is in use.

The current installer is unsigned. Windows can show an unknown-publisher or
SmartScreen warning; production distribution should use a trusted code-signing
certificate and timestamp. A SHA256 checksum verifies file integrity, not publisher
identity. Do not describe this build as signed or automatically installed.

## Maintainers

Install Inno Setup 6.3+ from [the official download page](https://jrsoftware.org/isdl.php).
Package a tested build, then compile:

```powershell
./tools/package-windows.ps1 -SkipBuild -BundleName Nuvyrec-Windows-Release -NativeTargetDir target/gpu-overlays
./tools/build-windows-installer.ps1 -BundleName Nuvyrec-Windows-Release
```

Use `-SkipBuild` only after building the current Flutter and native code. Supply
`-CompilerPath` if ISCC.exe is outside the default locations. The wrapper reads
the executable version, verifies every bundle hash and rejects unlisted files,
duplicates and escaping manifest paths. Output is the installer and `.sha256`
in `artifacts/github-release`. Existing installers are not overwritten.

Attach both files to the matching GitHub release tag. The updater already recognizes
`Nuvyrec-Windows*.exe` assets and opens their release page. Installation remains
manual. Configure and rebuild the default update repository before publishing a
build for a replacement repository; existing user overrides remain persisted.

Keep the AppId in `packaging/windows/frame.iss` unchanged across future versions.
Do not add broad uninstall deletion rules for user data or force-close processes.
Before public distribution, test install, launch, upgrade, active-app refusal and
uninstall on a clean supported Windows machine. Local smoke checks do not verify
clean-machine compatibility or hardware performance.

The earlier Frame 0.3.0 installer passed local integration checks: 54 installed file
hashes, installed application startup, same-version reinstall, refusal while the
destination executable is locked and uninstall registration/file cleanup. Run
`./tools/test-windows-installer.ps1` from a test account without a real Nuvyrec
installation to repeat this check. Its unique install directory and logs stay
under `artifacts`; the test installation is uninstalled even if a check fails.
