fn main() {
    // Standard Tauri build
    tauri_build::build();

    // Platform-specific native capture backend compilation
    #[cfg(target_os = "macos")]
    build_macos_backend();

    #[cfg(target_os = "windows")]
    build_windows_backend();

    #[cfg(target_os = "linux")]
    build_linux_backend();
}

#[cfg(target_os = "macos")]
fn build_macos_backend() {

    println!("cargo:rerun-if-changed=src/capture/backends/macos/sck_wrapper.mm");
    println!("cargo:rerun-if-changed=src/encoder/macos/vt_encoder.mm");
    println!("cargo:rerun-if-changed=src/capture/backends/macos/avcapture_wrapper.mm");

    // Compile ScreenCaptureKit wrapper
    cc::Build::new()
        .file("src/capture/backends/macos/sck_wrapper.mm")
        .flag("-std=c++17")
        .flag("-ObjC++")
        .flag("-fobjc-arc") // Enable ARC
        .flag("-fobjc-weak") // Allow weak refs where SDK needs it
        .flag("-mmacosx-version-min=12.3") // Match ScreenCaptureKit minimum
        .cpp(true)
        .compile("sck_wrapper");

    // Compile VideoToolbox encoder wrapper
    cc::Build::new()
        .file("src/encoder/macos/vt_encoder.mm")
        .flag("-std=c++17")
        .flag("-ObjC++")
        .flag("-fobjc-arc") // Enable ARC
        .flag("-mmacosx-version-min=10.15") // VideoToolbox is available earlier
        .cpp(true)
        .compile("vt_encoder");

    // Compile AVFoundation webcam capture wrapper
    cc::Build::new()
        .file("src/capture/backends/macos/avcapture_wrapper.mm")
        .flag("-std=c++17")
        .flag("-ObjC++")
        .flag("-fobjc-arc")
        .flag("-mmacosx-version-min=10.15")
        .cpp(true)
        .compile("avcapture_wrapper");

    // Link macOS frameworks
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
    println!("cargo:rustc-link-lib=framework=VideoToolbox");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=AppKit");

    // Set minimum deployment target for ScreenCaptureKit
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=12.3");
}

#[cfg(target_os = "windows")]
fn build_windows_backend() {
    // Windows DXGI backend - native Rust implementation
    // No C++ compilation needed, just ensure Windows SDK is available
    println!("cargo:rustc-link-lib=d3d11");
    println!("cargo:rustc-link-lib=dxgi");
    println!("cargo:rustc-link-lib=mfplat");
    println!("cargo:rustc-link-lib=mfreadwrite");
}

#[cfg(target_os = "linux")]
fn build_linux_backend() {
    // Linux PipeWire backend
    // Link PipeWire libraries
    println!("cargo:rustc-link-lib=pipewire-0.3");
    println!("cargo:rustc-link-lib=spa-0.2");
}