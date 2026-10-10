# Frame desktop app

Native Flutter Windows UI backed by the Rust recorder_core C ABI. Build with
`flutter build windows --release`, then bundle the matching Rust and FFmpeg DLLs
using tools/package-windows.ps1 from the repository root. Running the bare Flutter
executable without those native libraries shows an actionable engine-load error.

The main workspace has a one-frame-per-second native thumbnail preview, capture
source selection, file selection and fixed recording controls. Video/audio/format
options live in a separate Settings panel. Old preview images are evicted from
Flutter's image cache, and preview can be switched off to reduce resource use.
Settings → Appearance provides Dark and Light themes with a saved per-user
Windows preference. Appearance changes are independent of recording sessions.

The generated macOS runner currently has no native capture backend or Rust library
packaging. It is not a supported recording product yet.
