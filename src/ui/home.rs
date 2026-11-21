//! Home: the groups of the current view as a paged list (the sidebar picks the view).
//! With a group open, the same groups shrink to a rail beside its emails.

use dioxus::prelude::*;

use super::format::{check, fmt_date};
use super::{Filter, Sort, State, ask_delete, close_group, open_group, show_held};
use crate::classify::CATEGORIES;
use crate::groups::{Group, Verdict};
use crate::icons::icon;
use crate::queue::{self, Kind};

/// Category groups in display order, with their colour family.
pub const FAMILIES: [(&str, &str); 4] = [("Important", "imp"), ("Dev & services", "dev"), ("Updates", "upd"), ("Low priority", "low")];
/// Groups per page: a page fits the window without scrolling.
const PAGE: usize = 15;

/// Colour family of a category: one hue per category group.
pub fn hue(category: &str) -> &'static str {
    let group = CATEGORIES.iter().find(|c| c.name == category).map(|c| c.group);
    FAMILIES.iter().find(|(name, _)| Some(*name) == group).map_or("low", |(_, h)| h)
}

/// Verdict as a shape plus label: ■ can go, ◧ review, □ keep. Only "can go" is red.
pub fn verdict_mark(v: Option<Verdict>) -> Element {
    let (class, label) = match v {
        Some(v) => (v.as_str(), v.label()),
        None => ("pending", "Summarizing…"),
    };
    rsx! {
        span { class: "mark mark-{class}", span { class: "vs" } "{label}" }
    }
}

fn title(filter: &Filter) -> String {
    match filter {
        Filter::Category(c) => c.clone(),
        Filter::Verdict(v) => v.label().to_string(),
        _ => "All mail".to_string(),
    }
}

/// Number of pages for `n` items.
fn page_count(n: usize) -> usize {
    n.div_ceil(PAGE).max(1)
}

/// Page buttons to show: all of them when few, else a window around the current one.
fn page_window(current: usize, pages: usize) -> std::ops::Range<usize> {
    const SHOWN: usize = 7;
    if pages <= SHOWN {
        return 0..pages;
    }
    let start = current.saturating_sub(SHOWN / 2).min(pages - SHOWN);
    start..start + SHOWN
}

#[component]
pub fn Home() -> Element {
    let mut s = use_context::<State>();
    let groups = s.visible_groups();
    let held = s.held.read().len();
    let emails: usize = groups.iter().map(|g| g.ids.len()).sum();
    let gone: Vec<&Group> = groups.iter().filter(|g| g.verdict() == Some(Verdict::Delete)).collect();
    let go_emails: usize = gone.iter().map(|g| g.ids.len()).sum();
    let go_keys: Vec<String> = gone.iter().map(|g| g.key.clone()).collect();
    let selecting = *s.selecting.read();
    let pages = page_count(groups.len());
    let page = (*s.page.read()).min(pages - 1);
    let start = page * PAGE;
    let shown: Vec<Group> = groups.iter().skip(start).take(PAGE).cloned().collect();
    let sort = *s.sort.read();

    rsx! {
        div { class: "home",
            div { class: "home-head",
                ViewTitle { groups: groups.len(), emails }
                span { class: "spacer" }
                select {
                    class: "btn",
                    "aria-label": "Sort groups",
                    value: if sort == Sort::Newest { "newest" } else { "size" },
                    onchange: move |e| {
                        s.sort.set(if e.value() == "newest" { Sort::Newest } else { Sort::Size });
                        s.page.set(0);
                    },
                    option { value: "size", "Most emails" }
                    option { value: "newest", "Newest" }
                }
                if !go_keys.is_empty() && !selecting {
                    button {
                        class: "btn danger",
                        onclick: move |_| {
                            s.selecting.set(true);
                            s.selected_groups.write().extend(go_keys.iter().cloned());
                        },
                        {icon("trash")}
                        "Select {go_emails} that can go"
                    }
                }
                button {
                    class: if selecting { "btn primary" } else { "btn" },
                    title: "Pick groups to trash (Esc to leave)",
                    onclick: move |_| {
                        let on = !*s.selecting.peek();
                        s.selecting.set(on);
                        if !on {
                            s.selected_groups.write().clear();
                        }
                    },
                    {icon("check")}
                    if selecting { "Done" } else { "Select" }
                }
            }
            if held > 0 {
                div { class: "new-groups",
                    button { class: "btn primary small", onclick: move |_| show_held(s),
                        {icon("sparkle")}
                        if held == 1 { "Show 1 new group" } else { "Show {held} new groups" }
                    }
                }
            }
            if groups.is_empty() {
                Empty {}
            } else {
                GroupList { groups: shown }
                Pager { page, pages, start, shown: groups.len().min(start + PAGE), total: groups.len() }
            }
        }
        SelectionBar {}
    }
}

#[component]
fn Pager(page: usize, pages: usize, start: usize, shown: usize, total: usize) -> Element {
    let mut s = use_context::<State>();
    rsx! {
        footer { class: "pager",
            span { class: "muted", "{start + 1}–{shown} of {total} groups" }
            span { class: "spacer" }
            if pages > 1 {
                button { class: "icon", disabled: page == 0, "aria-label": "Previous page", onclick: move |_| s.page.set(page - 1), {icon("back")} }
                for p in page_window(page, pages) {
                    button {
                        class: if p == page { "pg on" } else { "pg" },
                        "aria-current": if p == page { "page" },
                        onclick: move |_| s.page.set(p),
                        "{p + 1}"
                    }
                }
                button { class: "icon flip", disabled: page + 1 >= pages, "aria-label": "Next page", onclick: move |_| s.page.set(page + 1), {icon("back")} }
            }
        }
    }
}

#[component]
fn GroupList(groups: Vec<Group>) -> Element {
    let s = use_context::<State>();
    let selecting = *s.selecting.read();
    let picked = s.selected_groups.read().clone();
    rsx! {
        div { class: "groups", role: "list", "aria-label": "Groups",
            for g in groups {
                GroupRow { key: "{g.key}", selected: picked.contains(&g.key), selecting, group: g }
            }
        }
    }
}

/// One row per group, the same in the paged list and the reading column (CSS lays it out on one
/// line or stacked). Each part has its own view-transition name, so opening or closing a group
/// moves every row and part from one layout to the other.
#[component]
fn GroupRow(group: Group, #[props(default)] selecting: bool, #[props(default)] selected: bool, #[props(default)] on: bool) -> Element {
    let mut s = use_context::<State>();
    let g = group.clone();
    let summary = group.summary.as_ref().map_or_else(|| format!("Latest: {}", group.subject), |x| x.summary.clone());
    let vt = transition_name(&group.key);
    rsx! {
        button {
            class: "grow hue-{hue(&group.category)}",
            class: if selected { "sel" },
            class: if on { "on" },
            role: "listitem",
            "aria-current": if on { "true" },
            title: "{group.category}",
            style: "view-transition-name: {vt}",
            onclick: move |_| {
                if selecting {
                    let mut sel = s.selected_groups.write();
                    if !sel.remove(&g.key) {
                        sel.insert(g.key.clone());
                    }
                } else {
                    open_group(s, &g);
                }
            },
            span { class: "gi", style: "view-transition-name: {vt}-i",
                if selecting { {check(selected, false)} } else { {icon(&group.category)} }
            }
            span { class: "who", style: "view-transition-name: {vt}-w",
                "{group.org} "
                span { "{group.topic}" }
            }
            span { class: "n", style: "view-transition-name: {vt}-n", "{group.ids.len()}" }
            span { class: "v", style: "view-transition-name: {vt}-v", {verdict_mark(group.verdict())} }
            span { class: "sum", style: "view-transition-name: {vt}-s", "{summary}" }
            span { class: "d", "{fmt_date(&group.last)}" }
        }
    }
}

/// A CSS-safe name unique to a group key (keys hold any characters).
fn transition_name(key: &str) -> String {
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    format!("g{hash:x}")
}

/// Title and counts of the current view: the same element above the list and above the column.
#[component]
fn ViewTitle(groups: usize, emails: usize) -> Element {
    let s = use_context::<State>();
    rsx! {
        div { class: "view-title",
            h1 { "{title(&s.filter.read())}" }
            span { class: "muted", "{emails} emails · {groups} groups" }
        }
    }
}

/// With a group open: the view's groups as a narrow column, so you can move between them while reading.
#[component]
pub fn GroupRail() -> Element {
    let s = use_context::<State>();
    let groups = s.visible_groups();
    let open = s.group.read().clone();
    let emails: usize = groups.iter().map(|g| g.ids.len()).sum();
    rsx! {
        section { class: "rail", "aria-label": "Groups",
            header { class: "rail-head",
                button { class: "btn small back", title: "Back to all groups (Esc)", onclick: move |_| close_group(s),
                    {icon("back")}
                    "All groups"
                }
                ViewTitle { groups: groups.len(), emails }
            }
            div { class: "rail-list", role: "list",
                for g in groups {
                    GroupRow { key: "{g.key}", on: open.as_ref() == Some(&g.key), group: g }
                }
            }
        }
    }
}

#[component]
fn SelectionBar() -> Element {
    let mut s = use_context::<State>();
    let picked = s.selected_groups.read().clone();
    if picked.is_empty() {
        return rsx! {};
    }
    let emails: usize = s.groups.read().iter().filter(|g| picked.contains(&g.key)).map(|g| g.ids.len()).sum();
    rsx! {
        div { class: "selection-bar",
            b { "{picked.len()} groups, {emails} emails selected" }
            span { class: "spacer" }
            button { class: "btn ghost", onclick: move |_| s.selected_groups.write().clear(), "Clear" }
            button { class: "btn", title: "Delete forever (Shift+Del)", onclick: move |_| ask_delete(s, s.targets()),
                {icon("deleteForever")}
                "Delete forever"
            }
            button {
                class: "btn danger",
                title: "Move to Trash (Del)",
                onclick: move |_| {
                    queue::enqueue(s, Kind::Trash, s.targets());
                    s.selecting.set(false);
                },
                {icon("trash")}
                "Move to Trash"
            }
        }
    }
}

#[component]
fn Empty() -> Element {
    let s = use_context::<State>();
    let (heading, text) = if !s.query.read().trim().is_empty() {
        ("No matches", "Nothing matches your search.")
    } else if *s.unanalyzed.read() > 0 {
        ("Reading your inbox…", "Groups appear here as emails are sorted.")
    } else if s.groups.read().is_empty() {
        ("No emails yet", "Press Sync to fetch and sort your inbox.")
    } else {
        ("Nothing here", "No groups match this view.")
    };
    rsx! {
        div { class: "empty",
            span { class: "empty-icon", {icon("inbox")} }
            strong { "{heading}" }
            span { "{text}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_names_are_css_safe_and_distinct() {
        let (a, b) = (transition_name("Zalando|promo tion/€"), transition_name("Zalando|promotion"));
        assert!(a.starts_with('g') && a.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(a, b);
    }

    #[test]
    fn pages_round_up_and_never_zero() {
        assert_eq!((page_count(0), page_count(1), page_count(PAGE), page_count(PAGE + 1)), (1, 1, 1, 2));
    }

    #[test]
    fn page_window_shows_all_or_centres_on_current() {
        assert_eq!(page_window(0, 3), 0..3);
        assert_eq!(page_window(0, 20), 0..7);
        assert_eq!(page_window(10, 20), 7..14);
        assert_eq!(page_window(19, 20), 13..20);
    }
}
