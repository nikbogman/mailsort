//! Filters (verdicts, categories, Trash), sync status and keyboard hints.

use dioxus::prelude::*;

use super::{Filter, State, SyncState, show, start_sync, totals};
use crate::classify::{CATEGORIES, Category};
use crate::groups::Verdict;
use crate::icons::icon;
use crate::sync::{self, Stage};

pub fn verdict_icon(v: Verdict) -> &'static str {
    match v {
        Verdict::Delete => "trash",
        Verdict::Review => "alert",
        Verdict::Keep => "bookmark",
    }
}

#[component]
pub fn Sidebar() -> Element {
    let s = use_context::<State>();
    let groups = s.groups.read();
    let (by_verdict, by_category) = totals(&groups);
    let total: usize = groups.iter().map(|g| g.ids.len()).sum();
    let mut sections: Vec<(&str, Vec<&Category>)> = vec![];
    for c in CATEGORIES {
        match sections.last_mut() {
            Some((group, cats)) if *group == c.group => cats.push(c),
            _ => sections.push((c.group, vec![c])),
        }
    }
    // Trash lives in Gmail, so its count is only known while viewing it.
    let trash_count = if s.in_trash() { Some(s.emails.read().len()) } else { None };

    rsx! {
        aside { class: "nav",
            div { class: "brand",
                span { class: "logo", {icon("mail")} }
                "mailsort"
            }
            NavItem { filter: Filter::All, label: "Overview", icon_name: "inbox", count: Some(total) }
            div { class: "nav-label", "Verdict" }
            for v in Verdict::ALL {
                NavItem {
                    filter: Filter::Verdict(v),
                    label: v.label(),
                    icon_name: verdict_icon(v),
                    count: Some(by_verdict.get(&v).copied().unwrap_or(0)),
                    class: "verdict-{v.as_str()}",
                }
            }
            for (section, cats) in sections {
                div { class: "nav-label", "{section}" }
                for c in cats {
                    NavItem {
                        filter: Filter::Category(c.name.to_string()),
                        label: c.name,
                        icon_name: c.name,
                        count: Some(by_category.get(c.name).copied().unwrap_or(0)),
                    }
                }
            }
            div { class: "nav-label", "Gmail" }
            NavItem { filter: Filter::Trash, label: "Trash", icon_name: "trash", count: trash_count }
            SyncPanel {}
        }
    }
}

#[component]
fn NavItem(
    filter: Filter,
    #[props(into)] label: String,
    #[props(into)] icon_name: String,
    count: Option<usize>,
    #[props(into, default)] class: String,
) -> Element {
    let s = use_context::<State>();
    let active = *s.filter.read() == filter;
    let zero = count == Some(0) && !active;
    rsx! {
        button {
            class: "nav-item {class}",
            class: if active { "active" },
            class: if zero { "zero" },
            onclick: move |_| show(s, filter.clone()),
            {icon(&icon_name)}
            span { class: "label", "{label}" }
            if let Some(n) = count {
                span { class: "count", "{n}" }
            }
        }
    }
}

#[component]
fn SyncPanel() -> Element {
    let s = use_context::<State>();
    let state = s.sync.read().clone();
    let running = matches!(state, SyncState::Running(..));
    let text = match &state {
        SyncState::Idle => "Not synced yet".to_string(),
        SyncState::Running(Stage::Fetching, _, 0, _) => "Checking Gmail…".to_string(),
        SyncState::Running(Stage::Fetching, done, total, _) => format!("Fetching {done} of {total}"),
        SyncState::Running(Stage::Analyzing, done, total, _) => format!("Analyzing {done} of {total}"),
        SyncState::Running(Stage::Summarizing, done, total, _) => format!("Summarizing {done} of {total} groups"),
        SyncState::Done(_) => "Up to date".to_string(),
        SyncState::Stopped => "Sync stopped".to_string(),
        SyncState::Failed(_) => "Sync failed".to_string(),
    };
    let eta = match state {
        SyncState::Running(_, _, _, Some(secs)) if secs < 60 => Some("less than a minute left".to_string()),
        SyncState::Running(_, _, _, Some(secs)) => Some(format!("about {} min left", secs.div_ceil(60))),
        _ => None,
    };
    let progress = match state {
        SyncState::Running(_, done, total, _) if total > 0 => Some(done * 100 / total),
        _ => None,
    };
    rsx! {
        div { class: "sync",
            div { class: "sync-row",
                div { class: "sync-status",
                    span { class: if matches!(state, SyncState::Failed(_)) { "sync-text failed" } else { "sync-text" }, "{text}" }
                    if let Some(eta) = eta {
                        span { class: "sync-eta", "{eta}" }
                    }
                }
                if running {
                    button { class: "btn ghost small", title: "Stop after the current email; progress is kept", onclick: move |_| sync::stop(),
                        {icon("x")}
                        "Stop"
                    }
                } else {
                    button { class: "btn ghost small", onclick: move |_| start_sync(s),
                        {icon("sync")}
                        "Sync"
                    }
                }
            }
            if let Some(p) = progress {
                div { class: "sync-bar", span { style: "width:{p}%" } }
            }
            if let SyncState::Failed(e) = &state {
                div { class: "sync-error", "{e}" }
            }
            div { class: "shortcuts",
                div { "Search" kbd { "/" } }
                div { "Trash" kbd { "Del" } }
                div {
                    "Delete forever"
                    span { kbd { "Shift" } " " kbd { "Del" } }
                }
            }
        }
    }
}
