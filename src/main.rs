// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod classify;
mod config;
mod db;
mod google;
mod groups;
mod icons;
mod queue;
mod sync;
mod ui;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

fn main() {
    let head = format!(
        r#"<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap">
<style>{}</style>"#,
        include_str!("../assets/style.css")
    );
    let window = WindowBuilder::new()
        .with_title("mailsort")
        .with_inner_size(LogicalSize::new(1320.0, 840.0))
        .with_min_inner_size(LogicalSize::new(900.0, 560.0));
    let config = Config::new().with_window(window).with_menu(None).with_custom_head(head);
    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(ui::App);
}
