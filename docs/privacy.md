# Nuvyrec privacy policy

Nuvyrec records the display/window/region, microphone, system audio and camera
selected by the operator. Recordings, exports, screenshots, saved layouts and
preferences remain on the local computer. The app has no analytics, advertising
SDK, account login or developer-operated recording upload service.

## Network connections

- **GitHub updates:** Automatic checks are enabled by default and run while the
  app is open and idle, at most daily. Turn them off in Settings → App updates.
  Manual Check now also contacts GitHub. Requests identify the configured public
  repository and the Nuvyrec update checker; GitHub sees the network IP and normal
  request metadata. No recording content or stream keys are included.
- **Live streaming:** Starting configured RTMP/RTMPS streaming sends the chosen
  video and audio to the destinations selected by the operator. Those platforms'
  own privacy policies apply. Stream credentials are held in app memory for the
  session; they are not written to saved app preferences.
- **Browser downloads:** Opening an update launches the default browser on a
  GitHub release page. GitHub and browser policies apply to downloads and logins.

GitHub's [privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement)
describes its handling of update requests and release downloads. Consult the
privacy policies of streaming destinations before enabling them.

## Local data and uninstall

Windows preferences use the existing per-user `Software\FrameScreenRecorder`
registry key for migration compatibility. Uninstall removes installed program
files and shortcuts while preserving preferences and user recordings. Users can
delete recordings/exports through their file manager and change preferences in
the app. No developer account is required to use local recording.

## Signing service

The proposed SignPath workflow submits built program files and CI origin metadata
for release signing. It does not receive user recordings or devices' captured
content. [SignPath privacy policy](https://about.signpath.io/privacy-policy) applies to
maintainers' signing accounts; this does not create a runtime SignPath connection
in the installed app.
