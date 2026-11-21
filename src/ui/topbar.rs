//! Top bar: logo, breadcrumb (where you are), sync status, search, Trash.

use dioxus::prelude::*;

use super::list::SearchBox;
use super::home::hue;
use super::{Filter, State, SyncState, close_group, show, start_sync};
use crate::icons::icon;
use crate::sync::{self, Stage};

/// The logo: a sheet of stamps, the red-crossed one peeling off. Both versions are inlined;
/// the stylesheet shows the one that matches the theme.
pub fn logo() -> Element {
    const DARK: &str = include_str!("../../assets/logo-dark.svg");
    const LIGHT: &str = include_str!("../../assets/logo-light.svg");
    rsx! {
        span { class: "logo logo-dark", "aria-hidden": "true", dangerous_inner_html: DARK }
        span { class: "logo logo-light", "aria-hidden": "true", dangerous_inner_html: LIGHT }
    }
}

#[component]
pub fn TopBar() -> Element {
    let s = use_context::<State>();
    let filter = s.filter.read().clone();
    let group = s.open_group();
    let trash = filter == Filter::Trash;
    let progress = match *s.sync.read() {
        SyncState::Running(_, done, total, _) if total > 0 => Some(done * 100 / total),
        _ => None,
    };

    rsx! {
        if let Some(p) = progress {
            div { class: "sync-line", span { style: "width:{p}%" } }
        }
        header { class: "topbar",
            button { class: "brand", title: "All mail", onclick: move |_| show(s, Filter::All),
                {logo()}
                "mailsort"
            }
            nav { class: "crumbs", "aria-label": "Breadcrumb",
                span { class: "sep", "/" }
                if filter == Filter::All && group.is_none() {
                    span { class: "here", "All mail" }
                } else {
                    button { class: "crumb", onclick: move |_| show(s, Filter::All), "All mail" }
                }
                match &filter {
                    Filter::Trash => rsx! {
                        span { class: "sep", "/" }
                        span { class: "here", "Trash" }
                    },
                    Filter::Verdict(v) => rsx! {
                        span { class: "sep", "/" }
                        span { class: "here", "{v.label()}" }
                    },
                    Filter::Category(c) => rsx! {
                        span { class: "sep", "/" }
                        if group.is_some() {
                            button { class: "crumb fg-{hue(c)}", onclick: move |_| close_group(s), {icon(c)} "{c}" }
                        } else {
                            span { class: "here fg-{hue(c)}", {icon(c)} "{c}" }
                        }
                    },
                    Filter::All => rsx! {},
                }
                if let Some(g) = &group {
                    span { class: "sep", "/" }
                    span { class: "here", "{g.org}" }
                }
            }
            span { class: "spacer" }
            UpdateButton {}
            SyncStatus {}
            SearchBox {}
            ThemeSwitch {}
            button {
                class: if trash { "icon on" } else { "icon" },
                title: "Trash",
                "aria-label": "Trash",
                onclick: move |_| show(s, Filter::Trash),
                {icon("trash")}
            }
        }
    }
}

#[component]
fn SyncStatus() -> Element {
    let s = use_context::<State>();
    let state = s.sync.read().clone();
    let running = matches!(state, SyncState::Running(..));
    let text = match &state {
        SyncState::Idle => "Not synced yet".to_string(),
        SyncState::Running(Stage::Fetching, _, 0, _) => "Checking Gmail…".to_string(),
        SyncState::Running(Stage::Fetching, done, total, _) => format!("Fetching {done} of {total}"),
        SyncState::Running(Stage::Analyzing, done, total, _) => format!("Reading {done} of {total}"),
        SyncState::Running(Stage::Summarizing, done, total, _) => format!("Summarizing {done} of {total} groups"),
        SyncState::Done(_) => "Up to date".to_string(),
        SyncState::Stopped => "Sync stopped".to_string(),
        SyncState::Failed(_) => "Sync failed".to_string(),
    };
    let eta = match state {
        SyncState::Running(_, _, _, Some(secs)) if secs < 60 => " · under a minute".to_string(),
        SyncState::Running(_, _, _, Some(secs)) => format!(" · about {} min", secs.div_ceil(60)),
        _ => String::new(),
    };
    let detail = if let SyncState::Failed(e) = &state { e.clone() } else { String::new() };
    rsx! {
        span { class: if matches!(state, SyncState::Failed(_)) { "sync-text failed" } else { "sync-text" }, title: "{detail}", "{text}{eta}" }
        if running {
            button { class: "btn small", title: "Stop after the current email; progress is kept", onclick: move |_| sync::stop(),
                {icon("x")}
                "Stop"
            }
        } else {
            button { class: "icon", title: "Sync now", "aria-label": "Sync now", onclick: move |_| start_sync(s), {icon("sync")} }
        }
    }
}

/// "Update to vX": shown when GitHub has a newer release. Desktop only.
#[component]
fn UpdateButton() -> Element {
    #[cfg(target_arch = "wasm32")]
    return rsx! {};
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut s = use_context::<State>();
        let release = use_resource(crate::update::check);
        let mut busy = use_signal(|| false);
        let Some(Some(r)) = release.read().clone() else { return rsx! {} };
        let tag = r.tag_name.clone();
        rsx! {
            button {
                class: "btn small",
                title: "Download the new version and restart",
                disabled: busy(),
                onclick: move |_| {
                    let r = r.clone();
                    busy.set(true);
                    spawn(async move {
                        if let Err(e) = crate::update::install(&r).await {
                            s.error.set(Some(e));
                            busy.set(false);
                        }
                    });
                },
                {icon("sync")}
                if busy() { "Updating…" } else { "Update to {tag}" }
            }
        }
    }
}

/// Light/dark switch. The theme lives on <html data-theme> and in localStorage; a script in the page head applies it at startup.
#[component]
fn ThemeSwitch() -> Element {
    const TOGGLE: &str = r#"
        const r = document.documentElement;
        const dark = r.dataset.theme ? r.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
        r.dataset.theme = dark ? "light" : "dark";
        try { localStorage.setItem("theme", r.dataset.theme); } catch (e) {}
    "#;
    rsx! {
        button {
            class: "icon theme-btn",
            title: "Light or dark theme",
            "aria-label": "Switch theme",
            onclick: move |_| {
                document::eval(TOGGLE);
            },
            span { class: "moon", {icon("moon")} }
            span { class: "sun", {icon("sun")} }
        }
    }
}
