//! Email list for an open group or Trash: toolbar, group summary, mails that open in place. Plus the shared search box.

use dioxus::prelude::*;

use super::format::{avatar, check, email_document, fmt_date, fmt_range, has_remote_images, sender_address, sender_name};
use super::home::{hue, verdict_mark};
use super::{State, ask_delete, close_group, read_email};
use crate::db::Email;
use crate::google;
use crate::groups::Verdict;
use crate::icons::icon;
use crate::queue::{self, Kind};

#[component]
pub fn SearchBox() -> Element {
    let mut s = use_context::<State>();
    rsx! {
        label { class: "search",
            {icon("search")}
            input {
                r#type: "search",
                placeholder: "Search",
                value: "{s.query}",
                oninput: move |e| s.query.set(e.value()),
                onkeydown: move |e| e.stop_propagation(), // typing here isn't a shortcut
                onmounted: move |e| s.search.set(Some(e.data())),
            }
            if s.query.read().is_empty() {
                kbd { "/" }
            }
        }
    }
}

#[component]
pub fn EmailToolbar() -> Element {
    let mut s = use_context::<State>();
    let view = s.visible_emails();
    let selected = s.selected.read().clone();
    let picked = view.iter().filter(|e| selected.contains(&e.id)).count();
    let all = !view.is_empty() && picked == view.len();
    let some = !selected.is_empty();
    let trash = s.in_trash();
    let group = s.open_group();
    let view_ids: Vec<String> = view.iter().map(|e| e.id.clone()).collect();
    let (all_ids, trash_ids) = (view_ids.clone(), view_ids.clone());

    rsx! {
        header { class: "toolbar",
            div { class: "toolbar-main",
                // Anything selected: clears. Nothing selected: selects all.
                span {
                    class: "check-hit",
                    title: if some { "Deselect all" } else { "Select all" },
                    onclick: move |_| {
                        let mut sel = s.selected.write();
                        if some { sel.clear() } else { sel.extend(all_ids.iter().cloned()) }
                    },
                    {check(all, some)}
                }
                if !selected.is_empty() {
                    h1 { "{selected.len()} selected" }
                    span { class: "spacer" }
                    if trash {
                        button { class: "btn", onclick: move |_| queue::enqueue(s, Kind::Restore, s.targets()),
                            {icon("restore")}
                            "Restore"
                        }
                        button { class: "btn danger", title: "Delete forever (Del)", onclick: move |_| ask_delete(s, s.targets()),
                            {icon("deleteForever")}
                            "Delete forever"
                        }
                    } else {
                        button { class: "icon danger", title: "Delete forever (Shift+Del)", "aria-label": "Delete forever", onclick: move |_| ask_delete(s, s.targets()),
                            {icon("deleteForever")}
                        }
                        button { class: "btn danger", title: "Move to Trash (Del)", onclick: move |_| queue::enqueue(s, Kind::Trash, s.targets()),
                            {icon("trash")}
                            "Trash"
                        }
                    }
                } else if trash {
                    h1 { "Trash" }
                    span { class: "total", "{view.len()}" }
                    span { class: "spacer" }
                    if !view.is_empty() {
                        button { class: "btn danger-ghost", onclick: move |_| ask_delete(s, view_ids.clone()),
                            {icon("deleteForever")}
                            "Empty trash"
                        }
                    }
                } else if let Some(g) = group {
                    button { class: "icon", title: "Back to all groups (Esc)", "aria-label": "Back to all groups", onclick: move |_| close_group(s), {icon("back")} }
                    {avatar(&g.org, "sm")}
                    h1 { "{g.org}" }
                    span { class: "total", "{view.len()}" }
                    span { class: "spacer" }
                    button { class: "icon danger", title: "Delete all forever", "aria-label": "Delete all forever", onclick: move |_| ask_delete(s, view_ids.clone()),
                        {icon("deleteForever")}
                    }
                    button { class: "btn danger", onclick: move |_| queue::enqueue(s, Kind::Trash, trash_ids.clone()),
                        {icon("trash")}
                        "Trash all"
                    }
                }
            }
        }
    }
}

/// The open group's header: category colour, verdict, summary and the span it covers.
#[component]
fn GroupSummary() -> Element {
    let s = use_context::<State>();
    let Some(g) = s.open_group() else { return rsx! {} };
    let h = hue(&g.category);
    rsx! {
        div { class: "group-head hue-{h}",
            if g.verdict() == Some(Verdict::Delete) {
                span { class: "gh-hatch" }
            }
            div { class: "gh-cat fg-{h}", {icon(&g.category)} "{g.category}" }
            h2 { "{g.org}" }
            if !g.topic.is_empty() {
                div { class: "gh-topic", "{g.topic}" }
            }
            div { class: "gh-meta",
                {verdict_mark(g.verdict())}
                span { "{g.ids.len()} emails, {fmt_range(&g.first, &g.last)}" }
            }
            if let Some(sum) = &g.summary {
                p { class: "summary", "{sum.summary}" }
                if !sum.reason.is_empty() {
                    p { class: "reason", "{sum.reason}" }
                }
            }
        }
    }
}

#[component]
pub fn EmailList() -> Element {
    let s = use_context::<State>();
    if *s.loading.read() {
        return rsx! {
            div { class: "list",
                div { class: "empty",
                    span { class: "spinner dark" }
                    "Loading Trash from Gmail…"
                }
            }
        };
    }
    let view = s.visible_emails();
    let selected = s.selected.read();
    let open = s.open.read().clone();
    let trash = s.in_trash();
    rsx! {
        div { class: "list",
            if !trash {
                GroupSummary {}
            }
            if view.is_empty() {
                div { class: "empty",
                    span { class: "empty-icon", {icon("inbox")} }
                    if trash {
                        strong { "Trash is empty" }
                    } else {
                        strong { "No matches" }
                    }
                }
            }
            for e in view {
                Mail {
                    key: "{e.id}",
                    selected: selected.contains(&e.id),
                    open: open.as_ref() == Some(&e.id),
                    trash,
                    email: e.clone(),
                }
            }
        }
    }
}

/// One email, collapsed in the list or open in place. Opening grows the card to show the body; closing
/// shrinks it back. The body stays mounted until the shrink ends, so both directions animate.
#[component]
fn Mail(email: Email, selected: bool, open: bool, trash: bool) -> Element {
    let mut s = use_context::<State>();
    // Body mounted: set on open, cleared once the close animation (--dur in style.css) is over.
    let mut shown = use_signal(|| open);
    use_effect(use_reactive!(|open| {
        if open {
            shown.set(true);
            spawn(async move {
                crate::sleep(std::time::Duration::from_millis(340)).await;
                document::eval("document.querySelector('.mail.open')?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })");
            });
        } else if *shown.peek() {
            spawn(async move {
                crate::sleep(std::time::Duration::from_millis(340)).await;
                shown.set(false);
            });
        }
    }));
    let id = email.id.clone();
    // Each handler owns its copy of the id.
    let (id_open, id_toggle, id_act, id_delete) = (id.clone(), id.clone(), id.clone(), id);
    let subject = if email.subject.is_empty() { "(no subject)".to_string() } else { email.subject.clone() };

    rsx! {
        article {
            class: "mail",
            class: if selected { "selected" },
            class: if open { "open" },
            div {
                class: "mail-head",
                onclick: move |_| read_email(s, if open { None } else { Some(id_open.clone()) }),
                span {
                    class: "check-hit",
                    onclick: move |e| {
                        e.stop_propagation();
                        let mut sel = s.selected.write();
                        if !sel.remove(&id_toggle) {
                            sel.insert(id_toggle.clone());
                        }
                    },
                    {check(selected, false)}
                }
                {avatar(&email.sender, "")}
                div { class: "main",
                    div { class: "line1",
                        span { class: "sender", "{sender_name(&email.sender)}" }
                        span { class: "date", "{fmt_date(&email.date)}" }
                    }
                    div { class: "subject", "{subject}" }
                    // Same line, two contents: the card's head never changes height.
                    div { class: "snippet",
                        if open {
                            "{sender_address(&email.sender)} · {email.date}"
                        } else {
                            "{email.snippet}"
                        }
                    }
                }
                div { class: "mail-actions",
                    button {
                        class: "icon",
                        title: if trash { "Restore" } else { "Move to Trash (Del)" },
                        onclick: move |e| {
                            e.stop_propagation();
                            let kind = if trash { Kind::Restore } else { Kind::Trash };
                            queue::enqueue(s, kind, vec![id_act.clone()]);
                        },
                        {icon(if trash { "restore" } else { "trash" })}
                    }
                    button {
                        class: "icon danger",
                        title: "Delete forever",
                        onclick: move |e| {
                            e.stop_propagation();
                            ask_delete(s, vec![id_delete.clone()]);
                        },
                        {icon("deleteForever")}
                    }
                }
            }
            div {
                class: "mail-more",
                div {
                    if open || shown() {
                        MailBody { email: email.clone() }
                    }
                }
            }
        }
    }
}

/// The open part: AI line and the sandboxed body. Own component, so shown images reset on each open.
#[component]
fn MailBody(email: Email) -> Element {
    let mut show_images = use_signal(|| false);
    let id = email.id.clone();
    let body = use_resource(move || {
        let id = id.clone();
        async move { google::get_body(&id).await }
    });

    let frame = match &*body.read() {
        Some(Ok((mime, text))) => rsx! {
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
        Some(Err(err)) => rsx! {
            div { class: "body-status", "Couldn't load this email: {err}" }
        },
        None => rsx! {
            div { class: "body-status", span { class: "spinner dark" } }
        },
    };

    rsx! {
        div { class: "mail-body",
            if !email.org.is_empty() {
                div { class: "ai",
                    {icon("sparkle")}
                    span {
                        b { "{email.org}" }
                        " · {email.topic} · {email.category}"
                    }
                }
            }
            div { class: "body-frame", {frame} }
        }
    }
}
