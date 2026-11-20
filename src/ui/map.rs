//! Home: the inbox as a treemap. At the top level each block is a category; inside a category each
//! block is a group. Block size = emails, colour = category group, red hatching = what can go.

use dioxus::prelude::*;

use super::treemap::{self, Rect};
use super::{Filter, State, ask_delete, open_group, show, show_held, totals};
use crate::classify::CATEGORIES;
use crate::groups::{Group, Verdict};
use crate::icons::icon;
use crate::queue::{self, Kind};

/// Map width / height; matches `.treemap { aspect-ratio }` in the stylesheet.
const ASPECT: f64 = 2.0;

/// Colour family of a category: one hue per category group.
pub fn hue(category: &str) -> &'static str {
    match CATEGORIES
        .iter()
        .find(|c| c.name == category)
        .map(|c| c.group)
    {
        Some("Important") => "imp",
        Some("Dev & services") => "dev",
        Some("Updates") => "upd",
        _ => "low",
    }
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
enum Target {
    Category(String),
    Group(Group),
    Unsorted,
}

#[derive(Clone, PartialEq)]
struct Block {
    target: Target,
    name: String,
    sub: String,
    n: usize,
    go: usize,
    hue: &'static str,
    icon: String,
    /// Group keys a click selects in select mode: the category's "can go" groups, or the group.
    keys: Vec<String>,
    tip: String,
}

fn blocks(s: &State, groups: &[Group]) -> Vec<Block> {
    let filter = s.filter.read().clone();
    if matches!(filter, Filter::Category(_)) {
        return groups
            .iter()
            .map(|g| Block {
                target: Target::Group(g.clone()),
                name: g.org.clone(),
                sub: g.topic.clone(),
                n: g.ids.len(),
                go: if g.verdict() == Some(Verdict::Delete) {
                    g.ids.len()
                } else {
                    0
                },
                hue: hue(&g.category),
                icon: g.category.clone(),
                keys: vec![g.key.clone()],
                tip: g
                    .summary
                    .as_ref()
                    .map_or_else(|| format!("Latest: {}", g.subject), |x| x.summary.clone()),
            })
            .collect();
    }
    let mut out = vec![];
    for c in CATEGORIES {
        let in_cat: Vec<&Group> = groups.iter().filter(|g| g.category == c.name).collect();
        if in_cat.is_empty() {
            continue;
        }
        let gone: Vec<&&Group> = in_cat
            .iter()
            .filter(|g| g.verdict() == Some(Verdict::Delete))
            .collect();
        out.push(Block {
            target: Target::Category(c.name.to_string()),
            name: c.name.to_string(),
            sub: if in_cat.len() == 1 {
                "1 group".into()
            } else {
                format!("{} groups", in_cat.len())
            },
            n: in_cat.iter().map(|g| g.ids.len()).sum(),
            go: gone.iter().map(|g| g.ids.len()).sum(),
            hue: hue(c.name),
            icon: c.name.to_string(),
            keys: gone.iter().map(|g| g.key.clone()).collect(),
            tip: format!("{}: {}", c.name, c.hint),
        });
    }
    let unsorted = *s.unanalyzed.read();
    if unsorted > 0 && filter == Filter::All && s.query.read().trim().is_empty() {
        out.push(Block {
            target: Target::Unsorted,
            name: "Still reading".into(),
            sub: "not sorted yet".into(),
            n: unsorted,
            go: 0,
            hue: "none",
            icon: "sync".into(),
            keys: vec![],
            tip: "Emails the local model hasn't sorted yet".into(),
        });
    }
    out
}

#[component]
pub fn MapView() -> Element {
    let s = use_context::<State>();
    let groups = s.visible_groups();
    let held = s.held.read().len();
    let list = blocks(&s, &groups);
    rsx! {
        div { class: "map-page",
            MapHeader {}
            if held > 0 {
                div { class: "new-groups",
                    button { class: "btn primary small", onclick: move |_| show_held(s),
                        {icon("sparkle")}
                        if held == 1 { "Show 1 new group" } else { "Show {held} new groups" }
                    }
                }
            }
            if list.is_empty() {
                Empty {}
            } else {
                Treemap { blocks: list }
                HueLegend {}
            }
        }
        SelectionBar {}
    }
}

#[component]
fn MapHeader() -> Element {
    let mut s = use_context::<State>();
    let filter = s.filter.read().clone();
    let (by_verdict, _) = totals(&s.groups.read());
    let groups = s.visible_groups();
    let emails: usize = groups.iter().map(|g| g.ids.len()).sum();
    let gone: Vec<&Group> = groups
        .iter()
        .filter(|g| g.verdict() == Some(Verdict::Delete))
        .collect();
    let go_emails: usize = gone.iter().map(|g| g.ids.len()).sum();
    let go_keys: Vec<String> = gone.iter().map(|g| g.key.clone()).collect();
    let selecting = *s.selecting.read();
    let (title, sub) = match &filter {
        Filter::Category(c) => (c.clone(), format!("{emails} emails in {} groups. Click a block to open it.", groups.len())),
        Filter::Verdict(v) => (v.label().to_string(), format!("{emails} emails in {} groups, by category.", groups.len())),
        _ => (
            "Where your mail is".to_string(),
            "Each block is a category, sized by how many emails it holds. The red hatching is what can go.".to_string(),
        ),
    };

    rsx! {
        div { class: "map-head",
            div { class: "map-title",
                h1 { "{title}" }
                p { "{sub}" }
            }
            // Fixed row under the title: the filter never moves, actions sit at the right.
            div { class: "map-bar",
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
                div { class: "map-actions",
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
                        title: "Pick blocks to trash (Esc to leave)",
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
}

#[component]
fn Treemap(blocks: Vec<Block>) -> Element {
    let s = use_context::<State>();
    let weights: Vec<f64> = blocks.iter().map(|b| b.n.max(1) as f64).collect();
    let rects = treemap::layout(&weights, ASPECT);
    let selecting = *s.selecting.read();
    let picked = s.selected_groups.read().clone();
    rsx! {
        div { class: if selecting { "treemap selecting" } else { "treemap" }, role: "list",
            for (b, r) in blocks.into_iter().zip(rects) {
                Tile {
                    key: "{b.name}|{b.sub}",
                    selected: !b.keys.is_empty() && b.keys.iter().all(|k| picked.contains(k)),
                    block: b,
                    rect: r,
                    selecting,
                }
            }
        }
    }
}

#[component]
fn Tile(block: Block, rect: Rect, selecting: bool, selected: bool) -> Element {
    let mut s = use_context::<State>();
    let area = rect.width * rect.height / 100.0; // percent of the map
    let size = if area > 9.0 {
        "big"
    } else if area < 1.8 {
        "tiny"
    } else {
        ""
    };
    let hatch = if block.n > 0 {
        block.go * 100 / block.n
    } else {
        0
    };
    let can_pick = !block.keys.is_empty();
    let is_group = matches!(block.target, Target::Group(_));
    let verdict = match &block.target {
        Target::Group(g) => Some(g.verdict()),
        _ => None,
    };
    let foot = match block.target {
        Target::Category(_) if block.go > 0 => format!("{} can go", block.go),
        Target::Category(_) => "nothing to clear".to_string(),
        Target::Unsorted => block.sub.clone(),
        Target::Group(_) => String::new(),
    };
    let b = block.clone();
    let onclick = move |_| {
        if selecting {
            if b.keys.is_empty() {
                return;
            }
            let mut sel = s.selected_groups.write();
            if b.keys.iter().all(|k| sel.contains(k)) {
                for k in &b.keys {
                    sel.remove(k);
                }
            } else {
                sel.extend(b.keys.iter().cloned());
            }
            return;
        }
        match &b.target {
            Target::Category(c) => show(s, Filter::Category(c.clone())),
            Target::Group(g) => open_group(s, g),
            Target::Unsorted => {}
        }
    };

    rsx! {
        div { class: "cell", style: "left:{rect.left}%;top:{rect.top}%;width:{rect.width}%;height:{rect.height}%",
            button {
                class: "tile hue-{block.hue} {size}",
                class: if block.target == Target::Unsorted { "unsorted" },
                class: if selected { "sel" },
                class: if selecting && !can_pick { "dim" },
                title: "{block.tip}",
                role: "listitem",
                onclick,
                if hatch > 0 {
                    span { class: "hatch", style: "height:{hatch}%" }
                }
                if selecting && can_pick {
                    span { class: if selected { "check checked" } else { "check" } }
                }
                span { class: "t-name", {icon(&block.icon)} "{block.name}" }
                if !block.sub.is_empty() && size != "tiny" && is_group {
                    span { class: "t-sub", "{block.sub}" }
                }
                span { class: "t-count", "{block.n}" }
                if size != "tiny" {
                    if let Some(v) = verdict {
                        span { class: "t-foot", {verdict_mark(v)} }
                    } else if !foot.is_empty() {
                        span { class: "t-foot", "{foot}" }
                    }
                }
            }
        }
    }
}

#[component]
fn HueLegend() -> Element {
    rsx! {
        div { class: "hue-legend",
            for (h , name) in [("imp", "Important"), ("dev", "Dev & services"), ("upd", "Updates"), ("low", "Low priority")] {
                span { span { class: "sw hue-{h}" } "{name}" }
            }
            span { span { class: "sw sw-hatch" } "can go" }
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
    let emails: usize = s
        .groups
        .read()
        .iter()
        .filter(|g| picked.contains(&g.key))
        .map(|g| g.ids.len())
        .sum();
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
        (
            "Reading your inbox…",
            "Blocks appear here as emails are sorted.",
        )
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
