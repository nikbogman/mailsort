//! Home: what's in the mailbox, grouped by (organization, topic), with summaries and verdicts.

use dioxus::prelude::*;

use super::format::{avatar, check, fmt_range};
use super::list::SearchBox;
use super::sidebar::verdict_icon;
use super::{Filter, State, ask_delete, open_group, show, show_held, totals};
use crate::classify::CATEGORIES;
use crate::groups::{Group, Verdict};
use crate::icons::icon;
use crate::queue::{self, Kind};

pub fn verdict_chip(v: Option<Verdict>, reason: &str) -> Element {
    match v {
        Some(v) => rsx! {
            span { class: "verdict verdict-{v.as_str()}", title: "{reason}",
                {icon(verdict_icon(v))}
                "{v.label()}"
            }
        },
        None => rsx! {
            span { class: "verdict pending", "Summarizing…" }
        },
    }
}

#[component]
pub fn OverviewToolbar() -> Element {
    let mut s = use_context::<State>();
    let groups = s.visible_groups();
    let picked = s.selected_groups.read().clone();
    let picked_emails: usize = groups.iter().filter(|g| picked.contains(&g.key)).map(|g| g.ids.len()).sum();
    let all = !groups.is_empty() && groups.iter().all(|g| picked.contains(&g.key));
    let some = !picked.is_empty();
    let keys: Vec<String> = groups.iter().map(|g| g.key.clone()).collect();
    let can_go: Vec<String> =
        groups.iter().filter(|g| g.verdict() == Some(Verdict::Delete) && !picked.contains(&g.key)).map(|g| g.key.clone()).collect();
    let emails: usize = groups.iter().map(|g| g.ids.len()).sum();
    let title = match &*s.filter.read() {
        Filter::All | Filter::Trash => "Overview".to_string(),
        Filter::Verdict(v) => v.label().to_string(),
        Filter::Category(c) => c.clone(),
    };

    rsx! {
        header { class: "toolbar",
            div { class: "toolbar-main",
                // Anything selected: clears. Nothing selected: selects all.
                span {
                    class: "check-hit",
                    title: if some { "Deselect all" } else { "Select all groups" },
                    onclick: move |_| {
                        let mut sel = s.selected_groups.write();
                        if some { sel.clear() } else { sel.extend(keys.iter().cloned()) }
                    },
                    {check(all, some)}
                }
                if !picked.is_empty() {
                    h1 { "{picked.len()} groups · {picked_emails} emails" }
                    button { class: "btn ghost", onclick: move |_| s.selected_groups.write().clear(), "Clear" }
                    span { class: "spacer" }
                    button { class: "btn danger-ghost", title: "Delete forever (Shift+Del)", onclick: move |_| ask_delete(s, s.targets()),
                        {icon("deleteForever")}
                        "Delete forever"
                    }
                    button { class: "btn danger", title: "Move to Trash (Del)", onclick: move |_| queue::enqueue(s, Kind::Trash, s.targets()),
                        {icon("trash")}
                        "Move to Trash"
                    }
                } else {
                    h1 { "{title}" }
                    span { class: "total", "{emails} emails · {groups.len()} groups" }
                    span { class: "spacer" }
                    if !can_go.is_empty() {
                        button {
                            class: "btn suggest",
                            onclick: move |_| s.selected_groups.write().extend(can_go.iter().cloned()),
                            {icon("trash")}
                            "Select {can_go.len()} that can go"
                        }
                    }
                }
            }
            SearchBox {}
        }
    }
}

#[component]
pub fn Overview() -> Element {
    let s = use_context::<State>();
    let groups = s.visible_groups();
    let selected = s.selected_groups.read().clone();
    let unanalyzed = *s.unanalyzed.read();
    let show_stats = *s.filter.read() == Filter::All && s.query.read().trim().is_empty();
    let held = s.held.read().len();

    if groups.is_empty() && held == 0 {
        let (heading, text) = if !s.query.read().trim().is_empty() {
            ("No matches", "Nothing matches your search.")
        } else if unanalyzed > 0 {
            ("Analyzing your inbox…", "Groups appear here as emails are analyzed.")
        } else if s.groups.read().is_empty() {
            ("No emails yet", "Press Sync to fetch and sort your inbox.")
        } else {
            ("Nothing here", "No groups match this filter.")
        };
        return rsx! {
            div { class: "list",
                div { class: "empty",
                    span { class: "empty-icon", {icon("inbox")} }
                    strong { "{heading}" }
                    span { "{text}" }
                }
            }
        };
    }

    rsx! {
        div { class: "list overview",
            if held > 0 {
                div { class: "new-groups",
                    button { class: "btn primary small", onclick: move |_| show_held(s),
                        {icon("sparkle")}
                        if held == 1 { "Show 1 new group" } else { "Show {held} new groups" }
                    }
                }
            }
            if show_stats {
                Stats {}
            }
            for c in CATEGORIES {
                {
                    // Already in display order: cards don't move while data updates.
                    let in_section: Vec<Group> = groups.iter().filter(|g| g.category == c.name).cloned().collect();
                    rsx! {
                        if !in_section.is_empty() {
                            Section { key: "{c.name}", name: c.name, groups: in_section, selected: selected.clone() }
                        }
                    }
                }
            }
        }
    }
}

/// "312 can go · 38 to review · 895 keep" as clickable tiles.
#[component]
fn Stats() -> Element {
    let s = use_context::<State>();
    let groups = s.groups.read();
    let (by_verdict, _) = totals(&groups);
    let group_count = |v: Verdict| groups.iter().filter(|g| g.verdict() == Some(v)).count();
    let unanalyzed = *s.unanalyzed.read();
    let unsummarized: usize = groups.iter().filter(|g| g.summary.is_none()).map(|g| g.ids.len()).sum();
    let waiting = unanalyzed + unsummarized;

    rsx! {
        div { class: "stats",
            for v in Verdict::ALL {
                button { class: "stat verdict-{v.as_str()}", onclick: move |_| show(s, Filter::Verdict(v)),
                    span { class: "stat-label", {icon(verdict_icon(v))} "{v.label()}" }
                    span { class: "stat-value", "{by_verdict.get(&v).copied().unwrap_or(0)}" }
                    span { class: "stat-sub", "emails in {group_count(v)} groups" }
                }
            }
            if waiting > 0 {
                div { class: "stat waiting",
                    span { class: "stat-label", {icon("sparkle")} "Still sorting" }
                    span { class: "stat-value", "{waiting}" }
                    span { class: "stat-sub", "emails being analyzed" }
                }
            }
        }
    }
}

#[component]
fn Section(#[props(into)] name: String, groups: Vec<Group>, selected: std::collections::HashSet<String>) -> Element {
    let s = use_context::<State>();
    let emails: usize = groups.iter().map(|g| g.ids.len()).sum();
    let can_go: Vec<String> = groups.iter().filter(|g| g.verdict() == Some(Verdict::Delete)).flat_map(|g| g.ids.clone()).collect();
    let n_can_go = can_go.len();

    rsx! {
        section { class: "section",
            div { class: "section-head",
                {icon(&name)}
                h2 { "{name}" }
                span { class: "muted", "{emails} emails · {groups.len()} groups" }
                span { class: "spacer" }
                if n_can_go > 0 {
                    button { class: "btn small danger-ghost", onclick: move |_| queue::enqueue(s, Kind::Trash, can_go.clone()),
                        {icon("trash")}
                        "Trash {n_can_go} that can go"
                    }
                }
            }
            div { class: "cards",
                for g in groups {
                    GroupCard { key: "{g.key}", selected: selected.contains(&g.key), group: g.clone() }
                }
            }
        }
    }
}

#[component]
fn GroupCard(group: Group, selected: bool) -> Element {
    let mut s = use_context::<State>();
    let n = group.ids.len();
    let (key, g_open, ids_trash, ids_delete) = (group.key.clone(), group.clone(), group.ids.clone(), group.ids.clone());
    let reason = group.summary.as_ref().map(|x| x.reason.clone()).unwrap_or_default();

    rsx! {
        div {
            class: "group-card",
            class: if selected { "selected" },
            onclick: move |_| open_group(s, &g_open),
            span {
                class: "check-hit",
                onclick: move |e| {
                    e.stop_propagation();
                    let mut sel = s.selected_groups.write();
                    if !sel.remove(&key) {
                        sel.insert(key.clone());
                    }
                },
                {check(selected, false)}
            }
            {avatar(&group.org, "")}
            div { class: "main",
                div { class: "line1",
                    span { class: "org", "{group.org}" }
                    span { class: "topic", "{group.topic}" }
                }
                match &group.summary {
                    Some(sum) => rsx! { div { class: "summary", "{sum.summary}" } },
                    None => rsx! { div { class: "summary muted", "Latest: {group.subject}" } },
                }
                div { class: "meta",
                    span { "{fmt_range(&group.first, &group.last)}" }
                    if !reason.is_empty() {
                        span { class: "dot", "·" }
                        span { class: "reason", "{reason}" }
                    }
                }
            }
            div { class: "side",
                span { class: "count", "{n}" }
                {verdict_chip(group.verdict(), &reason)}
            }
            div { class: "row-actions",
                button {
                    class: "icon",
                    title: "Move all {n} to Trash",
                    onclick: move |e| {
                        e.stop_propagation();
                        queue::enqueue(s, Kind::Trash, ids_trash.clone());
                    },
                    {icon("trash")}
                }
                button {
                    class: "icon danger",
                    title: "Delete all {n} forever",
                    onclick: move |e| {
                        e.stop_propagation();
                        ask_delete(s, ids_delete.clone());
                    },
                    {icon("deleteForever")}
                }
            }
        }
    }
}
