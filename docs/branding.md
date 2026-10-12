# Nuvyrec branding and release source

Product name: **Nuvyrec**. Public repository:
[dlemalshika/nuvyrec](https://github.com/dlemalshika/nuvyrec).
The first release is **1.0.0+5** (release tag `v1.0.0`). Earlier 0.3.x builds
were local development artifacts. The build number remains monotonic for those
development installs; the user-facing version is 1.0.0.

The owner supplied the neon PNG used at
`apps/desktop/assets/branding/nuvyrec-logo.png`. The source PNG is preserved.
`tools/generate-brand-icons.py` encodes padded square launcher variants without
cropping or changing the artwork. Windows ICO sizes range from 16 to 256 pixels;
macOS PNG assets are also prepared, but a macOS runtime build was not validated.

Windows executable, title, tray text, version resources, UI header, installer and
shortcuts use Nuvyrec. Existing native bridge IDs, settings registry keys and
installer AppId are intentionally stable so users retain their preferences.
The updater migrates the retired `dlemalshika/screenrecorder` source and clears
its cached release/check/skip state. Other configured repositories are retained.
The legacy `FRAME_UPDATE_REPOSITORY` build override still works.

Build, package and prepare the release assets after testing:

```powershell
cd apps/desktop
flutter build windows --release
cd ../..
./tools/package-windows.ps1 -SkipBuild -BundleName Nuvyrec-Windows-Release -NativeTargetDir target/gpu-overlays
./tools/prepare-github-release.ps1 -BundleName Nuvyrec-Windows-Release
./tools/build-windows-installer.ps1 -BundleName Nuvyrec-Windows-Release
```

The versioned portable ZIP and Setup.exe, with their respective SHA256 files,
are prepared under `artifacts/github-release`. These commands do not publish to
GitHub. Installer signing and clean-machine release QA remain separate work.
