// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// The web demo (src/demo/) uses only part of the backend.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

mod classify;
mod config;
// The web demo swaps Gmail, SQLite and sync for in-memory fakes.
#[cfg_attr(target_arch = "wasm32", path = "demo/db.rs")]
mod db;
#[cfg_attr(target_arch = "wasm32", path = "demo/google.rs")]
mod google;
mod groups;
mod icons;
mod queue;
#[cfg_attr(target_arch = "wasm32", path = "demo/sync.rs")]
mod sync;
mod ui;
#[cfg(not(target_arch = "wasm32"))]
mod update;

#[cfg(not(target_arch = "wasm32"))]
use dioxus::desktop::tao::window::Icon;
#[cfg(not(target_arch = "wasm32"))]
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

pub async fn sleep(d: std::time::Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(d).await;
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::sleep(d).await;
}

/// Window and taskbar icon: the dark logo as raw 64x64 RGBA, so no image decoder is needed.
/// `assets/icon-64.rgba` is rendered from `assets/logo-dark.svg` (same as `icon.ico`).
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> Option<Icon> {
    Icon::from_rgba(include_bytes!("../assets/icon-64.rgba").to_vec(), 64, 64).ok()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus::launch(ui::App);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let head = format!(
        r#"<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,600;12..96,700&family=Figtree:wght@400;500;600;700&display=swap">
<style>{}</style>
<script>try{{const t=localStorage.getItem("theme");if(t)document.documentElement.dataset.theme=t}}catch(e){{}}</script>"#,
        include_str!("../assets/style.css")
    );
    let window = WindowBuilder::new()
        .with_title("mailsort")
        .with_window_icon(app_icon())
        .with_inner_size(LogicalSize::new(1320.0, 860.0))
        .with_min_inner_size(LogicalSize::new(900.0, 560.0));
    // A fixed webview profile, so localStorage (the theme) survives restarts.
    let config = Config::new().with_data_directory(config::DATA_DIR.join("webview")).with_window(window).with_menu(None).with_custom_head(head);
    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(ui::App);
}
