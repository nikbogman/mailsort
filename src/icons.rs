//! 24×24 stroke icons (Feather style), keyed by category name plus a few UI icons.

use dioxus::prelude::*;

fn path(name: &str) -> &'static str {
    match name {
        "Personal" => "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2M12 3a4 4 0 1 0 0 8a4 4 0 1 0 0-8",
        "Work" => "M4 7h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2zM16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16",
        "Admin" => "M3 21h18M5 21V10M19 21V10M9 21V10M15 21V10M12 3l9 5H3z",
        "Security" => "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
        "Receipts" => "M6 2h12v20l-3-2-3 2-3-2-3 2zM9 7h6M9 11h6M9 15h4",
        "Dev Tools" => "M16 18l6-6-6-6M8 6l-6 6 6 6",
        "Alerts" => "M22 12h-4l-3 9L9 3l-3 9H2",
        "Services" => "M4 3h16v6H4zM4 15h16v6H4zM8 6h.01M8 18h.01",
        "Events" => "M5 4h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM16 2v4M8 2v4M3 10h18",
        "Social" => "M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2M9 3a4 4 0 1 0 0 8a4 4 0 1 0 0-8M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75",
        "Notifications" => "M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 0 1-3.46 0",
        "Newsletters" => "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6M16 13H8M16 17H8M10 9H8",
        "Promotions" => "M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82zM7 7h.01",
        "Spam" => "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20M4.93 4.93l14.14 14.14",
        "trash" => "M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6",
        "deleteForever" => "M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11l4 4M14 11l-4 4",
        "restore" => "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5",
        "back" => "M15 18l-6-6 6-6",
        "mail" => "M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM22 6l-10 7L2 6",
        "search" => "M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14M21 21l-4.3-4.3",
        "sparkle" => "M12 3l1.9 5.8L20 11l-6.1 2.2L12 19l-1.9-5.8L4 11l6.1-2.2z",
        "inbox" => "M22 12h-6l-2 3h-4l-2-3H2M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
        "sync" => "M21 12a9 9 0 0 1-15.5 6.2L3 16M3 12a9 9 0 0 1 15.5-6.2L21 8M21 3v5h-5M3 21v-5h5",
        "check" => "M20 6L9 17l-5-5",
        "alert" => "M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z",
        "bookmark" => "M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z",
        "sun" => "M12 7a5 5 0 1 0 0 10a5 5 0 1 0 0-10M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42",
        "moon" => "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z",
        "x" => "M18 6L6 18M6 6l12 12",
        "folder" | _ => "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z",
    }
}

pub fn icon(name: &str) -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: path(name) }
        }
    }
}
