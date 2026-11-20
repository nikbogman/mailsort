//! First-run screen: asks for the Google OAuth credentials.json.

use dioxus::prelude::*;

use crate::config::DATA_DIR;
use crate::google;
use crate::icons::icon;

#[component]
pub fn Setup(on_ready: EventHandler) -> Element {
    let mut missing = use_signal(|| false);
    rsx! {
        div { class: "setup",
            div { class: "setup-card",
                span { class: "logo big", {icon("mail")} }
                h2 { "Connect your Gmail" }
                p {
                    "Put the "
                    code { "credentials.json" }
                    " you downloaded from Google Cloud (Desktop app client) into this folder:"
                }
                code { class: "path", "{DATA_DIR.display()}" }
                div { class: "dialog-actions",
                    button {
                        class: "btn",
                        onclick: move |_| {
                            #[cfg(not(target_arch = "wasm32"))]
                            let _ = open::that(&*DATA_DIR);
                        },
                        "Open folder"
                    }
                    button {
                        class: "btn primary",
                        onclick: move |_| {
                            if google::has_credentials() { on_ready.call(()) } else { missing.set(true) }
                        },
                        "Continue"
                    }
                }
                if missing() {
                    p { class: "error-text", "credentials.json isn't in that folder yet." }
                }
            }
        }
    }
}
