fn main() {
    println!("cargo:rerun-if-changed=native/wgc_capture.cpp");
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let sdk =
        std::env::var("FFMPEG_DIR").expect("FFMPEG_DIR must point to the FFmpeg development SDK");
    cc::Build::new()
        .cpp(true)
        .file("native/wgc_capture.cpp")
        .include(std::path::Path::new(&sdk).join("include"))
        .flag("/std:c++20")
        .flag("/EHsc")
        .flag("/permissive-")
        .define("NOMINMAX", None)
        .compile("frame_wgc");
    for library in [
        "windowsapp",
        "runtimeobject",
        "d3d11",
        "dxgi",
        "d3dcompiler",
        "dwmapi",
        "user32",
    ] {
        println!("cargo:rustc-link-lib={library}");
    }
}
