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

/// Window and taskbar icon: a tiny inbox map (same drawing as `topbar::logo`), drawn here so no
/// image file or decoder is needed.
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> Option<Icon> {
    const N: u32 = 64;
    const R: u32 = 12; // corner radius
    // (x0, y0, x1, y1, colour); later blocks paint over earlier ones.
    let blocks: [(u32, u32, u32, u32, [u8; 3]); 5] = [
        (6, 6, 36, 58, [0xc9, 0xb4, 0x8c]),
        (6, 40, 36, 58, [0xd9, 0x36, 0x36]),
        (38, 6, 58, 32, [0x2f, 0xb3, 0xa3]),
        (38, 34, 47, 58, [0x5b, 0x8d, 0xef]),
        (49, 34, 58, 58, [0x9b, 0x7b, 0xea]),
    ];
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let dx = if x < R { R - x } else if x >= N - R { x + R + 1 - N } else { 0 };
            let dy = if y < R { R - y } else if y >= N - R { y + R + 1 - N } else { 0 };
            if dx * dx + dy * dy > R * R {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let c = blocks.iter().rev().find(|b| x >= b.0 && x < b.2 && y >= b.1 && y < b.3).map_or([0x1b, 0x1d, 0x22], |b| b.4);
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Icon::from_rgba(rgba, N, N).ok()
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
