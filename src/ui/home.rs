//! Home: the inbox as a spectrum. One bar splits all mail by category (colour = category group);
//! below it, every group as a row. Clicking a category in the bar narrows the list to it.

use dioxus::prelude::*;

use super::format::{check, fmt_date};
use super::{Filter, State, ask_delete, open_group, show, show_held, totals};
use crate::classify::CATEGORIES;
use crate::groups::{Group, Verdict};
use crate::icons::icon;
use crate::queue::{self, Kind};

/// Category groups in bar order, with their colour family.
const FAMILIES: [(&str, &str); 4] = [("Important", "imp"), ("Dev & services", "dev"), ("Updates", "upd"), ("Low priority", "low")];
/// Inside a family, each smaller category is a lighter step of the same colour.
const STEPS: [f64; 5] = [1.0, 0.78, 0.6, 0.47, 0.37];

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

#[derive(Clone, PartialEq)]
struct Segment {
    category: String,
    n: usize,
    go: usize,
    opacity: f64,
}

#[derive(Clone, PartialEq)]
struct Family {
    name: &'static str,
    hue: &'static str,
    n: usize,
    segments: Vec<Segment>,
}

/// The bar: categories with mail, grouped by family, biggest first inside each family.
fn families(groups: &[Group]) -> Vec<Family> {
    let mut out = vec![];
    for (name, hue) in FAMILIES {
        let mut segments: Vec<Segment> = CATEGORIES
            .iter()
            .filter(|c| c.group == name)
            .map(|c| {
                let in_cat = groups.iter().filter(|g| g.category == c.name);
                let n = in_cat.clone().map(|g| g.ids.len()).sum();
                let go = in_cat.filter(|g| g.verdict() == Some(Verdict::Delete)).map(|g| g.ids.len()).sum();
                Segment { category: c.name.to_string(), n, go, opacity: 1.0 }
            })
            .filter(|s| s.n > 0)
            .collect();
        segments.sort_by_key(|s| std::cmp::Reverse(s.n));
        for (i, s) in segments.iter_mut().enumerate() {
            s.opacity = STEPS[i.min(STEPS.len() - 1)];
        }
        let n = segments.iter().map(|s| s.n).sum();
        if n > 0 {
            out.push(Family { name, hue, n, segments });
        }
    }
    out
}

#[component]
pub fn Home() -> Element {
    let s = use_context::<State>();
    let groups = s.visible_groups();
    let held = s.held.read().len();
    rsx! {
        div { class: "home",
            HomeHeader {}
            Spectrum {}
            FilterBar {}
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
                GroupList { groups }
            }
        }
        SelectionBar {}
    }
}

/// The big number: everything, one category, or one verdict.
#[component]
fn HomeHeader() -> Element {
    let s = use_context::<State>();
    let filter = s.filter.read().clone();
    let groups = s.visible_groups();
    let emails: usize = groups.iter().map(|g| g.ids.len()).sum();
    let go: usize = groups.iter().filter(|g| g.verdict() == Some(Verdict::Delete)).map(|g| g.ids.len()).sum();
    let categories = families(&s.groups.read()).iter().map(|f| f.segments.len()).sum::<usize>();
    let text = match &filter {
        Filter::Category(c) => format!("emails in {c}, {go} can go"),
        Filter::Verdict(Verdict::Delete) => "emails that can go".to_string(),
        Filter::Verdict(Verdict::Review) => "emails to review".to_string(),
        Filter::Verdict(Verdict::Keep) => "emails to keep".to_string(),
        _ => format!("emails in your inbox, sorted into {categories} categories"),
    };
    let unsorted = *s.unanalyzed.read();
    rsx! {
        div { class: "home-total",
            h1 { "{emails}" }
            span { "{text}" }
            if unsorted > 0 {
                span { class: "unsorted", {icon("sync")} "{unsorted} still being read" }
            }
        }
    }
}

/// One bar of the whole inbox by category. Always all mail, so it never jumps; the open category is outlined.
#[component]
fn Spectrum() -> Element {
    let s = use_context::<State>();
    let filter = s.filter.read().clone();
    let fams = families(&s.groups.read());
    if fams.is_empty() {
        return rsx! {};
    }
    let open = match &filter {
        Filter::Category(c) => Some(c.clone()),
        _ => None,
    };
    rsx! {
        div { class: "spectrum", role: "group", "aria-label": "Categories by size",
            for f in fams.clone() {
                div { class: "fam hue-{f.hue}", style: "flex:{f.n}",
                    for seg in f.segments {
                        Seg { key: "{seg.category}", open: open.clone(), seg }
                    }
                }
            }
        }
        div { class: "fam-labels",
            for f in fams {
                span { class: "hue-{f.hue}", style: "flex:{f.n}",
                    {icon(&f.segments[0].category)}
                    b { "{f.name}" }
                    em { "{f.n}" }
                }
            }
        }
    }
}

#[component]
fn Seg(seg: Segment, open: Option<String>) -> Element {
    let s = use_context::<State>();
    let state = match &open {
        Some(c) if *c == seg.category => "on",
        Some(_) => "dim",
        None => "",
    };
    let tip = format!("{}: {} emails, {} can go", seg.category, seg.n, seg.go);
    let category = seg.category.clone();
    rsx! {
        button {
            class: "seg {state}",
            style: "flex:{seg.n};opacity:{seg.opacity}",
            title: "{tip}",
            "aria-label": "{tip}",
            // Clicking the open category again goes back to all mail.
            onclick: move |_| {
                let again = matches!(&*s.filter.peek(), Filter::Category(c) if *c == category);
                show(s, if again { Filter::All } else { Filter::Category(category.clone()) });
            },
        }
    }
}

/// Verdict filter on the left, actions on the right; fixed so nothing moves when clicked.
#[component]
fn FilterBar() -> Element {
    let mut s = use_context::<State>();
    let filter = s.filter.read().clone();
    let (by_verdict, _) = totals(&s.groups.read());
    let groups = s.visible_groups();
    let gone: Vec<&Group> = groups.iter().filter(|g| g.verdict() == Some(Verdict::Delete)).collect();
    let go_emails: usize = gone.iter().map(|g| g.ids.len()).sum();
    let go_keys: Vec<String> = gone.iter().map(|g| g.key.clone()).collect();
    let selecting = *s.selecting.read();
    rsx! {
        div { class: "filter-bar",
            div { class: "legend", role: "group", "aria-label": "Show by verdict",
                button {
                    class: if filter == Filter::All { "lg on" } else { "lg" },
                    onclick: move |_| show(s, Filter::All),
                    "All"
                }
                for v in Verdict::ALL {
                    button {
                        class: if filter == Filter::Verdict(v) { "lg on" } else { "lg" },
                        onclick: move |_| show(s, Filter::Verdict(v)),
                        span { class: "mark mark-{v.as_str()}",
                            span { class: "vs" }
                            "{by_verdict.get(&v).copied().unwrap_or(0)} {v.label().to_lowercase()}"
                        }
                    }
                }
            }
            div { class: "filter-actions",
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
            div { class: "groups-head",
                span {}
                span { "From" }
                span { "Summary" }
                span { class: "r", "Emails" }
                span { "Verdict" }
                span { class: "r", "Latest" }
            }
            for g in groups {
                GroupRow { key: "{g.key}", selected: picked.contains(&g.key), selecting, group: g }
            }
        }
    }
}

#[component]
fn GroupRow(group: Group, selecting: bool, selected: bool) -> Element {
    let mut s = use_context::<State>();
    let g = group.clone();
    let summary = group.summary.as_ref().map_or_else(|| format!("Latest: {}", group.subject), |x| x.summary.clone());
    rsx! {
        button {
            class: "grow hue-{hue(&group.category)}",
            class: if selected { "sel" },
            role: "listitem",
            title: "{group.category}",
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
            span { class: "gi",
                if selecting { {check(selected, false)} } else { {icon(&group.category)} }
            }
            span { class: "who",
                "{group.org} "
                span { "{group.topic}" }
            }
            span { class: "sum", "{summary}" }
            span { class: "n", "{group.ids.len()}" }
            span { {verdict_mark(group.verdict())} }
            span { class: "d", "{fmt_date(&group.last)}" }
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
    use crate::groups::Summary;

    fn group(category: &str, n: usize, verdict: Verdict) -> Group {
        Group {
            key: format!("{category}{n}"),
            org: String::new(),
            topic: String::new(),
            category: category.into(),
            ids: (0..n).map(|i| format!("{category}{i}")).collect(),
            sender: String::new(),
            subject: String::new(),
            first: String::new(),
            last: String::new(),
            summary: Some(Summary { summary: String::new(), verdict, reason: String::new(), fingerprint: String::new() }),
        }
    }

    #[test]
    fn bar_groups_categories_by_family_biggest_first() {
        let fams = families(&[
            group("Spam", 5, Verdict::Delete),
            group("Work", 3, Verdict::Keep),
            group("Newsletters", 20, Verdict::Delete),
            group("Newsletters", 4, Verdict::Review),
        ]);
        let names: Vec<&str> = fams.iter().map(|f| f.name).collect();
        assert_eq!(names, ["Important", "Low priority"]); // empty families are left out, order is fixed
        let low = &fams[1];
        assert_eq!(low.n, 29);
        let segs: Vec<(&str, usize, usize, f64)> = low.segments.iter().map(|s| (s.category.as_str(), s.n, s.go, s.opacity)).collect();
        assert_eq!(segs, [("Newsletters", 24, 20, 1.0), ("Spam", 5, 5, 0.78)]);
    }
}
