# Mobile Flutter host (planned)

Generate the platform runners in Phase 2 with:

```powershell
flutter create --platforms=android,ios --org org.screenrecorder --project-name recorder_mobile .
```

Use a federated native capture plugin for consent, services and texture preview.
Android uses Kotlin plus JNI to Rust. iOS uses Swift plus the C ABI, with a
separately signed ReplayKit broadcast extension for cross-app recording. The
extension must record without a Flutter engine and cannot share process-local
handles with the containing app. There is no runnable mobile app in Phase 1.
