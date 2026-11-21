//! Reading pane: header, AI verdict, and the sandboxed email body.

use dioxus::prelude::*;

use super::format::{avatar, email_document, has_remote_images, sender_address, sender_name};
use super::{State, ask_delete, read_email};
use crate::google;
use crate::icons::icon;
use crate::queue::{self, Kind};

/// Keyed by email id in the parent, so per-email state (shown images) resets on each email.
#[component]
pub fn Reader() -> Element {
    let s = use_context::<State>();
    let mut show_images = use_signal(|| false);
    let body = use_resource(move || async move {
        let id = s.open.read().clone()?;
        Some(google::get_body(&id).await)
    });
    let Some(id) = s.open.read().clone() else { return rsx! {} };
    let Some(e) = s.emails.read().iter().find(|e| e.id == id).cloned() else { return rsx! {} };
    let trash = s.in_trash();
    let subject = if e.subject.is_empty() { "(no subject)".to_string() } else { e.subject.clone() };
    let (id_delete, id_act) = (id.clone(), id);

    let frame = match &*body.read() {
        Some(Some(Ok((mime, text)))) => rsx! {
            if !show_images() && has_remote_images(mime, text) {
                div { class: "images-bar",
                    span { "Images are hidden so the sender can't see you opened this." }
                    button { class: "btn small", onclick: move |_| show_images.set(true), "Show images" }
                }
            }
            iframe {
                "sandbox": "allow-top-navigation-by-user-activation",
                "srcdoc": "{email_document(mime, text, show_images())}",
                title: "Email content",
            }
        },
        Some(Some(Err(err))) => rsx! {
            div { class: "body-status", "Couldn't load this email: {err}" }
        },
        _ => rsx! {
            div { class: "body-status", span { class: "spinner dark" } }
        },
    };

    rsx! {
        section { class: "reader",
            div { class: "reader-bar",
                button { class: "icon", title: "Close (Esc)", onclick: move |_| read_email(s, None), {icon("back")} }
                span { class: "actions",
                    button { class: "btn danger-ghost", onclick: move |_| ask_delete(s, vec![id_delete.clone()]),
                        {icon("deleteForever")}
                        "Delete forever"
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            let kind = if trash { Kind::Restore } else { Kind::Trash };
                            queue::enqueue(s, kind, vec![id_act.clone()]);
                        },
                        {icon(if trash { "restore" } else { "trash" })}
                        if trash { "Restore" } else { "Trash" }
                    }
                }
            }
            div { class: "reader-head",
                h2 { "{subject}" }
                div { class: "from",
                    {avatar(&e.sender, "lg")}
                    div {
                        div { class: "name ellipsis", "{sender_name(&e.sender)}" }
                        div { class: "addr ellipsis", "{sender_address(&e.sender)}" }
                    }
                    span { class: "when", "{e.date}" }
                }
                if !e.org.is_empty() {
                    div { class: "ai",
                        {icon("sparkle")}
                        span {
                            b { "{e.org}" }
                            " · {e.topic} · {e.category}"
                        }
                    }
                }
            }
            div { class: "body-frame", {frame} }
        }
    }
}
