//! Starts mudcrab's GUI; the app itself is in `lib.rs`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's DMA-BUF renderer dies on NVIDIA under Wayland ("Error 71
    // (Protocol error) dispatching to Wayland display"); without it, it draws
    // fine. Found by the phase 3 spike (docs/design/gui.md); Tauri's docs give
    // the same workaround.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: nothing else is running yet to read the environment.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    mudcrab::ui::init("gui", false, None);
    mudcrab_gui::app(tauri::Builder::default(), std::env::args().nth(1), true)
        .run(tauri::generate_context!())
        .expect("the window opens");
}
