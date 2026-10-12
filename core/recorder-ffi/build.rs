//! Give our Windows DLL the same product/version identity as the Flutter app.
//! SignPath enforces this metadata before signing project-owned binaries.
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../../apps/desktop/pubspec.yaml");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let pubspec = fs::read_to_string("../../apps/desktop/pubspec.yaml").expect("read app version");
    let version = pubspec
        .lines()
        .find_map(|line| line.strip_prefix("version: "))
        .expect("pubspec version")
        .trim();
    assert!(
        version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".+-".contains(c)),
        "invalid version"
    );
    let (release, build) = version.split_once('+').unwrap_or((version, "0"));
    let numbers: Vec<u16> = release
        .split('-')
        .next()
        .unwrap()
        .split('.')
        .map(|part| part.parse().expect("numeric version component"))
        .collect();
    assert_eq!(numbers.len(), 3);
    let build: u16 = build.parse().expect("numeric build number");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let resource = output.join("nuvyrec-version.rc");
    fs::write(
        &resource,
        format!(
            r#"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},{build}
PRODUCTVERSION {major},{minor},{patch},{build}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0
FILEOS 0x40004L
FILETYPE 0x2L
FILESUBTYPE 0
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904e4"
  BEGIN
   VALUE "CompanyName", "Nuvyrec contributors\0"
   VALUE "FileDescription", "Nuvyrec recording engine\0"
   VALUE "FileVersion", "{version}\0"
   VALUE "InternalName", "recorder_core\0"
   VALUE "OriginalFilename", "recorder_core.dll\0"
   VALUE "ProductName", "Nuvyrec\0"
   VALUE "ProductVersion", "{version}\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x409, 1252
 END
END
"#,
            major = numbers[0],
            minor = numbers[1],
            patch = numbers[2]
        ),
    )
    .expect("write version resource");
    // cargo can invoke rustc outside a Developer Prompt. Locate the SDK's x64
    // resource compiler explicitly.
    let sdk = PathBuf::from(env::var_os("ProgramFiles(x86)").expect("ProgramFiles(x86)"))
        .join("Windows Kits/10/bin");
    let mut candidates: Vec<_> = fs::read_dir(sdk)
        .expect("Windows SDK")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("x64/rc.exe"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    let compiler = candidates.pop().expect("Windows SDK rc.exe");
    let compiled = output.join("nuvyrec-version.res");
    let status = Command::new(compiler)
        .arg("/nologo")
        .arg("/fo")
        .arg(&compiled)
        .arg(resource)
        .status()
        .expect("run resource compiler");
    assert!(status.success(), "resource compiler failed");
    println!("cargo:rustc-link-arg-cdylib={}", compiled.display());
}
