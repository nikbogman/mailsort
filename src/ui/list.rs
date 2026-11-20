//! Email list for an open group or Trash: toolbar, group summary, rows. Plus the shared search box.

use dioxus::prelude::*;

use super::format::{avatar, check, fmt_date, fmt_range, sender_name};
use super::map::{hue, verdict_mark};
use super::{State, ask_delete, close_group};
use crate::db::Email;
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
                if !trash {
                    button { class: "icon", title: "Back to overview (Esc)", onclick: move |_| close_group(s), {icon("back")} }
                }
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
                Row {
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

#[component]
fn Row(email: Email, selected: bool, open: bool, trash: bool) -> Element {
    let mut s = use_context::<State>();
    let id = email.id.clone();
    // Each handler owns its copy of the id.
    let (id_open, id_toggle, id_act, id_delete) = (id.clone(), id.clone(), id.clone(), id);
    let subject = if email.subject.is_empty() { "(no subject)".to_string() } else { email.subject.clone() };

    rsx! {
        div {
            class: "row",
            class: if selected { "selected" },
            class: if open { "open" },
            onclick: move |_| s.open.set(Some(id_open.clone())),
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
                div { class: "snippet", "{email.snippet}" }
            }
            div { class: "row-actions",
                button {
                    class: "icon",
                    title: if trash { "Restore" } else { "Move to Trash" },
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
    }
}
