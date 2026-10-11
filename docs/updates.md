# GitHub updates (Windows)

Frame 0.3.0 adds **Settings → App updates** (the update icon beside Close).
The settings list also has an Updates panel. The default source is
`dlemalshika/screenrecorder`, verified against this checkout's Git remote.
Enter a public `owner/repository` or GitHub repository URL to change it.
Builds can override it with `--dart-define=FRAME_UPDATE_REPOSITORY=owner/repository`.

Stable checks GitHub `/releases/latest`. Beta examines up to 100 recent releases
and selects the highest valid version, including stable versions. Drafts, bad
tags, older versions and build-number-only changes do not trigger notifications.
Bare tags and commits are not releases. See [GitHub Releases API](https://docs.github.com/en/rest/releases/releases)
and [SemVer precedence](https://semver.org/).

The installed version comes from the Windows runner, including Flutter
`--build-name` overrides. Repository/channel, automatic-check preference, cached
release metadata, Later and Skip persist with existing user preferences.
Changing repository/channel clears its cache and skip state. **Check now** also
works when automatic checks are off, with no-release/offline messages.

Automatic checks start after 15 seconds and run at most once per day, with an
hourly due-time poll while Frame is open and idle. Active and scheduled
recordings defer background requests. Failed automatic checks wait until the
next day. Manual checks remain available. There is no background helper while
Frame is closed, OS push notification, login, token or capture/stream telemetry.
Requests have timeouts, no redirects and a 4 MiB response budget. No new package
dependency was needed.

## User flow

A newer release adds an **Update available** card beside capture controls.
**View update** opens plain-text release notes. **Later** hides the card for a
day; **Skip this version** suppresses that tag across restarts. A skipped release
remains accessible manually in Settings.

**Download on GitHub** opens the repository's release page in the system browser.
Without a `Frame-Windows*.zip/.exe/.msi` asset, the button says **View release on
GitHub**. The native host accepts only GitHub release-page HTTPS URLs. It never
launches an installer, arbitrary protocol or local executable.

Stop/save recording, exit Frame, download the whole ZIP and extract into a new
folder. Run Frame.exe with the folder intact. Existing per-user settings remain.
Use the supplied SHA256 file to check the ZIP if desired. **Automatic verified
downloads, installers, signed updates, rollback and one-click installation are
not implemented in this revision.**

## Publish an update

1. Set a new pubspec version, for example `0.3.1+4` or `0.4.0-beta.1+5`.
   Test, build and package it.
2. Run `tools/prepare-github-release.ps1 -BundleName Frame-Windows-Updates`.
   The script verifies bundle hashes, reads the built executable version and
   prepares the full ZIP plus SHA256 file under `artifacts/github-release`.
   It refuses to replace an existing archive and never publishes anything.
3. Create a release in the public repository with matching `v0.3.1` tag, attach
   both files and write release notes. Mark Beta as prerelease; mark the intended
   Stable release as latest.
4. Checker-enabled older apps see it on their next successful check. Older apps
   without a checker must download this version manually once.

The development read-only check received HTTP 404 for this repository's latest
release. It may have no public stable release or be private/unavailable; the
response does not distinguish these. Nothing was published by this task.
GitHub Actions release automation remains separate work.

## Validation

Tests cover SemVer and malformed versions, source URL validation, channel/draft
filtering, daily throttle, Later/Skip persistence, offline/no-release states,
changed-source/disposal races, recording deferral and compact dark/light UI.
Browser launch is mocked; no real release is installed. End-to-end notification
requires publishing a release newer than the installed version.
