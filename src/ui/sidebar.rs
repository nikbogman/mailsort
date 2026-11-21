//! Left sidebar: every view one click away. All mail, the three verdicts, each category under its
//! family (with how many can go), and Trash.

use dioxus::prelude::*;

use super::home::FAMILIES;
use super::{Filter, State, show, totals};
use crate::classify::CATEGORIES;
use crate::groups::Verdict;
use crate::icons::icon;

#[component]
pub fn Sidebar() -> Element {
    let s = use_context::<State>();
    let groups = s.groups.read();
    let (by_verdict, by_category) = totals(&groups);
    let total: usize = groups.iter().map(|g| g.ids.len()).sum();
    // "Can go" per category, for the red counts.
    let mut go = std::collections::HashMap::<&str, usize>::new();
    for g in groups.iter().filter(|g| g.verdict() == Some(Verdict::Delete)) {
        *go.entry(g.category.as_str()).or_default() += g.ids.len();
    }
    let unsorted = *s.unanalyzed.read();
    let filter = s.filter.read().clone();

    rsx! {
        nav { class: "sidebar", "aria-label": "Views",
            Item { target: Filter::All, on: filter == Filter::All, label: "All mail", n: total, glyph: "inbox" }
            for v in Verdict::ALL {
                Item {
                    target: Filter::Verdict(v),
                    on: filter == Filter::Verdict(v),
                    label: v.label(),
                    n: by_verdict.get(&v).copied().unwrap_or(0),
                    mark: v.as_str(),
                }
            }
            if unsorted > 0 {
                div { class: "side-note", {icon("sync")} "{unsorted} still being read" }
            }
            for (family, h) in FAMILIES {
                // Only categories with mail; a family with none is left out.
                if CATEGORIES.iter().any(|c| c.group == family && by_category.get(c.name).copied().unwrap_or(0) > 0) {
                    div { class: "side-fam", "{family}" }
                    for c in CATEGORIES.iter().filter(|c| c.group == family) {
                        if let Some(&n) = by_category.get(c.name).filter(|&&n| n > 0) {
                            Item {
                                target: Filter::Category(c.name.to_string()),
                                on: filter == Filter::Category(c.name.to_string()),
                                label: c.name,
                                n,
                                go: go.get(c.name).copied().unwrap_or(0),
                                glyph: c.name,
                                hue: h,
                            }
                        }
                    }
                }
            }
            span { class: "spacer" }
            Item { target: Filter::Trash, on: filter == Filter::Trash, label: "Trash", n: 0, glyph: "trash" }
        }
    }
}

#[component]
fn Item(
    target: Filter,
    on: bool,
    label: &'static str,
    n: usize,
    #[props(default)] go: usize,
    #[props(default)] glyph: &'static str,
    #[props(default)] mark: &'static str,
    #[props(default)] hue: &'static str,
) -> Element {
    let s = use_context::<State>();
    rsx! {
        button {
            class: "side-item hue-{hue}",
            class: if on { "on" },
            class: if mark == "delete" { "go" },
            onclick: move |_| show(s, target.clone()),
            if !mark.is_empty() {
                span { class: "mark mark-{mark}", span { class: "vs" } }
            } else {
                span { class: "side-ic", {icon(glyph)} }
            }
            span { class: "side-label", "{label}" }
            if go > 0 {
                b { class: "side-go", title: "{go} can go", "{go}" }
            }
            if n > 0 {
                em { "{n}" }
            }
        }
    }
}
