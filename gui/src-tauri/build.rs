//! Tauri's build step, with the Windows manifest embedded by hand.
//!
//! The dialog plugin needs version 6 of Windows' common controls, which only a
//! manifest asks for. Tauri embeds one in the app itself but not in the test
//! binaries, which then die on start with STATUS_ENTRYPOINT_NOT_FOUND. Linking
//! it ourselves puts it in every binary, tests included -- Tauri's own repo
//! does the same.

fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri's build step");
}
