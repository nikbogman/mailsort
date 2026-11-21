//! App state, data loading, and the root component.
//!
//! A sidebar lists every view (all mail, verdicts, categories, Trash). Home is the view's groups (organization,
//! topic), each with a summary and verdict, a page at a time. Opening a group keeps the groups in a column beside
//! its emails and the reader.
//!
//! Updates while you look at the screen: rows keep their place (`order`); new groups found by a
//! running sync wait in `held` behind a "Show N new groups" button instead of pushing rows around.

mod format;
mod list;
mod home;
mod overlays;
mod reader;
mod setup;
mod sidebar;
mod topbar;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use web_time::Instant;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::db::{self, Email};
use crate::google;
use crate::groups::{self, Group, Verdict};
use crate::icons;
use crate::queue::{self, Job, Kind};
use crate::sync::{self, Stage};

/// Order of the group list.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sort {
    /// Most emails first; keeps its place while a sync adds mail.
    Size,
    Newest,
}

/// What the overview shows (or Trash, which is a plain email list from Gmail).
#[derive(Clone, PartialEq, Debug)]
pub enum Filter {
    All,
    Verdict(Verdict),
    Category(String),
    Trash,
}

#[derive(Clone, PartialEq, Default)]
pub enum SyncState {
    #[default]
    Idle,
    /// Stage, done, total, estimated seconds left.
    Running(Stage, usize, usize, Option<u64>),
    Done(usize),
    Stopped,
    Failed(String),
}

/// App-wide state, shared through context. Every field is a signal, so this is Copy.
#[derive(Clone, Copy)]
pub struct State {
    pub filter: Signal<Filter>,
    /// Key of the group being viewed.
    pub group: Signal<Option<String>>,
    /// Groups on screen.
    pub groups: Signal<Vec<Group>>,
    /// Display order of group keys; stays put while data updates.
    pub order: Signal<Vec<String>>,
    /// New groups found during a sync, not shown yet.
    pub held: Signal<Vec<Group>>,
    /// Emails of the open group, or of Trash.
    pub emails: Signal<Vec<Email>>,
    /// Last Trash listing, shown instantly while a fresh one loads.
    pub trash_cache: Signal<Option<Vec<Email>>>,
    /// Inbox emails not analyzed yet.
    pub unanalyzed: Signal<usize>,
    pub loading: Signal<bool>,
    pub selected: Signal<HashSet<String>>,
    pub selected_groups: Signal<HashSet<String>>,
    /// Select mode: clicking a group row selects it instead of opening it.
    pub selecting: Signal<bool>,
    /// Page of the group list (0-based) and its sort.
    pub page: Signal<usize>,
    pub sort: Signal<Sort>,
    /// Email open in the reader.
    pub open: Signal<Option<String>>,
    pub query: Signal<String>,
    pub jobs: Signal<Vec<Job>>,
    /// Ids waiting for "Delete forever?" confirmation.
    pub confirm: Signal<Option<Vec<String>>>,
    pub sync: Signal<SyncState>,
    pub error: Signal<Option<String>>,
    pub search: Signal<Option<Rc<MountedData>>>,
}

impl State {
    pub fn in_trash(&self) -> bool {
        *self.filter.read() == Filter::Trash
    }

    pub fn in_trash_peek(&self) -> bool {
        *self.filter.peek() == Filter::Trash
    }

    fn open_group(&self) -> Option<Group> {
        let key = self.group.read().clone()?;
        self.groups.read().iter().find(|g| g.key == key).cloned()
    }

    fn query_text(&self) -> String {
        self.query.read().trim().to_lowercase()
    }

    /// Groups matching the filter and search box, in display order.
    fn visible_groups(&self) -> Vec<Group> {
        let filter = self.filter.read().clone();
        let q = self.query_text();
        let position: HashMap<String, usize> = self.order.read().iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
        let mut groups: Vec<Group> = self
            .groups
            .read()
            .iter()
            .filter(|g| match &filter {
                Filter::All | Filter::Trash => true,
                Filter::Verdict(v) => g.verdict() == Some(*v),
                Filter::Category(c) => &g.category == c,
            })
            .filter(|g| {
                let summary = g.summary.as_ref().map_or("", |s| &s.summary);
                q.is_empty() || format!("{} {} {}", g.org, g.topic, summary).to_lowercase().contains(&q)
            })
            .cloned()
            .collect();
        groups.sort_by_key(|g| position.get(&g.key).copied().unwrap_or(usize::MAX));
        if *self.sort.read() == Sort::Newest {
            groups.sort_by(|a, b| b.last.cmp(&a.last));
        }
        groups
    }

    /// Emails of the open group or Trash, narrowed by the search box.
    fn visible_emails(&self) -> Vec<Email> {
        let q = self.query_text();
        self.emails
            .read()
            .iter()
            .filter(|e| q.is_empty() || format!("{} {} {}", e.sender, e.subject, e.snippet).to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    fn has_selection(&self) -> bool {
        !self.selected.peek().is_empty() || !self.selected_groups.peek().is_empty()
    }

    fn clear_selection(mut self) {
        self.selected.write().clear();
        self.selected_groups.write().clear();
    }

    /// What Delete acts on: selected emails, the open email, or the emails of selected groups.
    fn targets(&self) -> Vec<String> {
        if *self.filter.peek() == Filter::Trash || self.group.peek().is_some() {
            let selected = self.selected.peek();
            return if selected.is_empty() { self.open.peek().iter().cloned().collect() } else { selected.iter().cloned().collect() };
        }
        let picked = self.selected_groups.peek();
        self.groups.peek().iter().filter(|g| picked.contains(&g.key)).flat_map(|g| g.ids.clone()).collect()
    }

    /// Drop emails from everything on screen (they're being trashed or deleted).
    pub fn forget(mut self, gone: &HashSet<String>) {
        self.emails.write().retain(|e| !gone.contains(&e.id));
        if let Some(cache) = self.trash_cache.write().as_mut() {
            cache.retain(|e| !gone.contains(&e.id));
        }
        drop_ids(&mut self.groups.write(), gone);
        drop_ids(&mut self.held.write(), gone);
        self.selected.write().retain(|id| !gone.contains(id));
        let live: HashSet<String> = self.groups.peek().iter().map(|g| g.key.clone()).collect();
        self.selected_groups.write().retain(|k| live.contains(k));
        if self.open.peek().as_ref().is_some_and(|id| gone.contains(id)) {
            self.open.set(None);
        }
        if self.group.peek().as_ref().is_some_and(|k| !live.contains(k)) {
            self.group.set(None); // group emptied: back to the overview
        }
    }
}

fn drop_ids(groups: &mut Vec<Group>, gone: &HashSet<String>) {
    for g in groups.iter_mut() {
        g.ids.retain(|id| !gone.contains(id));
    }
    groups.retain(|g| !g.ids.is_empty());
}

/// Biggest groups first.
fn default_order(groups: &[Group]) -> Vec<String> {
    let mut sorted: Vec<&Group> = groups.iter().collect();
    sorted.sort_by_key(|g| std::cmp::Reverse(g.ids.len()));
    sorted.into_iter().map(|g| g.key.clone()).collect()
}

/// Reloads can overlap (sync progress, queue, view changes); only the newest one may apply.
static RELOADS: AtomicU64 = AtomicU64::new(0);

/// Refresh what's on screen without moving cards around.
pub async fn reload(s: State) {
    refresh(s, false).await
}

/// Refresh and re-sort everything (on a view change).
async fn refresh(mut s: State, resort: bool) {
    let generation = RELOADS.fetch_add(1, Ordering::SeqCst) + 1;
    let filter = s.filter.peek().clone();
    let group = s.group.peek().clone();
    let mut fresh = groups::load();
    let mut emails = if filter == Filter::Trash {
        match sync::trash_emails().await {
            Ok(trash) => trash,
            Err(e) => {
                s.error.set(Some(e));
                s.trash_cache.peek().clone().unwrap_or_default()
            }
        }
    } else if let Some(key) = &group {
        fresh.iter().find(|g| &g.key == key).map(|g| db::by_ids(&g.ids)).unwrap_or_default()
    } else {
        vec![]
    };
    if RELOADS.load(Ordering::SeqCst) != generation || *s.filter.peek() != filter || *s.group.peek() != group {
        return; // a newer reload or a view change superseded this one
    }

    // Emails still waiting in the queue stay hidden.
    let pending: HashSet<String> = s.jobs.peek().iter().filter(|j| j.is_pending()).flat_map(|j| j.ids.clone()).collect();
    drop_ids(&mut fresh, &pending);
    emails.retain(|e| !pending.contains(&e.id));

    let syncing = matches!(*s.sync.peek(), SyncState::Running(..));
    let (order, shown, held) = merge(s.order.peek().clone(), fresh, resort, syncing);
    let live: HashSet<String> = shown.iter().map(|g| g.key.clone()).collect();

    let ids: HashSet<String> = emails.iter().map(|e| e.id.clone()).collect();
    s.selected.write().retain(|id| ids.contains(id));
    if s.open.peek().as_ref().is_some_and(|id| !ids.contains(id)) {
        s.open.set(None);
    }
    s.selected_groups.write().retain(|k| live.contains(k));
    if group.as_ref().is_some_and(|k| !live.contains(k)) && filter != Filter::Trash {
        s.group.set(None);
    }
    if filter == Filter::Trash {
        s.trash_cache.set(Some(emails.clone()));
    }
    s.order.set(order);
    s.groups.set(shown);
    s.held.set(held);
    s.emails.set(emails);
    s.unanalyzed.set(db::unanalyzed_count());
    s.loading.set(false);
}

/// Fit freshly loaded groups into the current display order: known groups keep their place,
/// new ones are held back while syncing (else appended), vanished ones drop out.
/// Returns (order, shown, held).
fn merge(mut order: Vec<String>, fresh: Vec<Group>, resort: bool, syncing: bool) -> (Vec<String>, Vec<Group>, Vec<Group>) {
    if resort || order.is_empty() {
        return (default_order(&fresh), fresh, vec![]);
    }
    let known: HashSet<String> = order.iter().cloned().collect();
    let (mut shown, new): (Vec<Group>, Vec<Group>) = fresh.into_iter().partition(|g| known.contains(&g.key));
    let held = if syncing {
        new
    } else {
        order.extend(default_order(&new));
        shown.extend(new);
        vec![]
    };
    let live: HashSet<&String> = shown.iter().map(|g| &g.key).collect();
    order.retain(|k| live.contains(k));
    (order, shown, held)
}

/// "Show N new groups": append held groups after the ones on screen.
fn show_held(mut s: State) {
    let held = std::mem::take(&mut *s.held.write());
    s.order.write().extend(default_order(&held));
    s.groups.write().extend(held);
}

fn show(s: State, filter: Filter) {
    animate(move || {
        let mut s = s;
        let trash = filter == Filter::Trash;
        s.filter.set(filter);
        s.group.set(None);
        s.open.set(None);
        s.selecting.set(false);
        s.page.set(0);
        s.clear_selection();
        // Trash comes from Gmail and takes a moment: show the last listing meanwhile.
        let cached = if trash { s.trash_cache.peek().clone() } else { None };
        s.loading.set(trash && cached.is_none());
        s.emails.set(cached.unwrap_or_default());
        spawn_forever(refresh(s, true));
    });
}

fn open_group(s: State, g: &Group) {
    let (key, ids) = (g.key.clone(), g.ids.clone());
    animate(move || {
        let mut s = s;
        s.group.set(Some(key));
        s.open.set(None);
        s.selected.write().clear();
        s.emails.set(db::by_ids(&ids));
    });
}

fn close_group(s: State) {
    animate(move || {
        let mut s = s;
        s.group.set(None);
        s.open.set(None);
        s.selected.write().clear();
        s.emails.write().clear();
    });
}

/// Open an email (or close it with `None`).
fn read_email(mut s: State, id: Option<String>) {
    animate(move || s.open.set(id));
}

/// Runs a view change inside a View Transition: the browser snapshots the old layout, the change
/// renders, then it morphs one into the other (the moving parts are named in style.css). Waits
/// for `.shell[data-view]` to change before letting the new layout show. No API or reduced
/// motion: the change just happens.
fn animate(change: impl FnOnce() + 'static) {
    const JS: &str = r#"
        // The clicked button may be gone after the change: give focus back to the app so Esc still works.
        setTimeout(() => { if (document.activeElement === document.body) document.querySelector(".app")?.focus({ preventScroll: true }); }, 350);
        const shell = document.querySelector(".shell");
        if (!document.startViewTransition || !shell || matchMedia("(prefers-reduced-motion: reduce)").matches) {
            dioxus.send(true);
        } else {
            const before = shell.dataset.view;
            document.startViewTransition(() => new Promise(done => {
                const finish = () => {
                    watch.disconnect(); clearTimeout(late);
                    document.querySelector(".rail .grow.on")?.scrollIntoView({ block: "nearest" }); // the open group in view
                    done();
                };
                const watch = new MutationObserver(() => { if (shell.dataset.view !== before) finish(); });
                watch.observe(shell, { attributes: true, attributeFilter: ["data-view"] });
                const late = setTimeout(finish, 300); // nothing changed (same view): don't hang
                dioxus.send(true);
            }));
        }
    "#;
    spawn(async move {
        let _ = document::eval(JS).recv::<bool>().await;
        change();
    });
}

/// How often a running sync may refresh the screen.
const SYNC_REFRESH: Duration = Duration::from_millis(2500);

fn start_sync(mut s: State) {
    if matches!(*s.sync.peek(), SyncState::Running(..)) {
        return;
    }
    s.sync.set(SyncState::Running(Stage::Fetching, 0, 0, None));
    spawn_forever(async move {
        let mut sync_state = s.sync;
        let mut stage_started = (Stage::Fetching, Instant::now());
        let mut last_refresh = Instant::now();
        let result = sync::sync(|stage, done, total| {
            if stage != stage_started.0 {
                stage_started = (stage, Instant::now());
            }
            // Time left from this stage's pace so far, once there's enough to go on.
            let elapsed = stage_started.1.elapsed().as_secs_f64();
            let eta = (done >= 3 && done < total).then(|| ((total - done) as f64 * elapsed / done as f64) as u64);
            sync_state.set(SyncState::Running(stage, done, total, eta));
            if done == total || last_refresh.elapsed() >= SYNC_REFRESH {
                last_refresh = Instant::now();
                spawn_forever(reload(s));
            }
        })
        .await;
        s.sync.set(match result {
            Ok(n) => SyncState::Done(n),
            Err(e) if e == sync::STOPPED => SyncState::Stopped,
            Err(e) => SyncState::Failed(e),
        });
        reload(s).await; // sync over: held groups slot in
    });
}

fn ask_delete(mut s: State, ids: Vec<String>) {
    if !ids.is_empty() {
        s.confirm.set(Some(ids));
    }
}

/// Email totals per verdict and per category, for the home header.
fn totals(groups: &[Group]) -> (HashMap<Verdict, usize>, HashMap<String, usize>) {
    let mut by_verdict = HashMap::new();
    let mut by_category = HashMap::new();
    for g in groups {
        if let Some(v) = g.verdict() {
            *by_verdict.entry(v).or_default() += g.ids.len();
        }
        *by_category.entry(g.category.clone()).or_default() += g.ids.len();
    }
    (by_verdict, by_category)
}

#[component]
pub fn App() -> Element {
    let s = State {
        filter: use_signal(|| Filter::All),
        group: use_signal(|| None),
        groups: use_signal(Vec::new),
        order: use_signal(Vec::new),
        held: use_signal(Vec::new),
        emails: use_signal(Vec::new),
        trash_cache: use_signal(|| None),
        unanalyzed: use_signal(|| 0),
        loading: use_signal(|| false),
        selected: use_signal(HashSet::new),
        selected_groups: use_signal(HashSet::new),
        selecting: use_signal(|| false),
        open: use_signal(|| None),
        query: use_signal(String::new),
        jobs: use_signal(Vec::new),
        confirm: use_signal(|| None),
        sync: use_signal(SyncState::default),
        error: use_signal(|| None),
        search: use_signal(|| None),
        page: use_signal(|| 0),
        sort: use_signal(|| Sort::Size),
    };
    use_context_provider(|| s);
    let mut ready = use_signal(google::has_credentials);
    use_hook(move || {
        spawn_forever(reload(s));
        if *ready.peek() {
            start_sync(s);
        }
    });

    if !ready() {
        return rsx! {
            setup::Setup {
                on_ready: move |_| {
                    ready.set(true);
                    start_sync(s);
                },
            }
        };
    }

    let open = s.open.read().clone();
    // Changes whenever the layout does; `animate` waits for it.
    let view = format!("{:?}|{:?}|{:?}", s.filter.read(), s.group.read(), open);
    rsx! {
        div { class: "app", tabindex: "0", autofocus: true, onkeydown: move |e| on_key(s, e),
            topbar::TopBar {}
            overlays::ErrorBanner {}
            div { class: "shell", "data-view": "{view}",
                sidebar::Sidebar {}
                if s.in_trash() {
                    // Trash: list and reader side by side.
                    main { class: "content",
                        section { class: "list-pane",
                            list::EmailToolbar {}
                            list::EmailList {}
                        }
                        if let Some(id) = open {
                            reader::Reader { key: "{id}" }
                        } else {
                            div { class: "reader-empty",
                                span { class: "empty-icon", {icons::icon("mail")} }
                                "Select an email to read it"
                            }
                        }
                    }
                } else if s.group.read().is_some() {
                    // Reading: the groups stay in a column; the open group's emails (or one email) fill the rest.
                    main { class: "columns",
                        home::GroupRail {}
                        section { class: "detail",
                            if let Some(id) = open {
                                reader::Reader { key: "{id}" }
                            } else {
                                list::EmailToolbar {}
                                list::EmailList {}
                            }
                        }
                    }
                } else {
                    main { class: "home-scroll", home::Home {} }
                }
            }
        }
        overlays::Queue {}
        overlays::ConfirmDialog {}
    }
}

fn on_key(mut s: State, e: Event<KeyboardData>) {
    if s.confirm.peek().is_some() {
        if e.key() == Key::Escape {
            s.confirm.set(None);
        }
        return;
    }
    match e.key() {
        // Esc steps back one level: reader, selection, select mode, group, then back to all mail.
        Key::Escape if s.open.peek().is_some() => read_email(s, None),
        Key::Escape if s.has_selection() => s.clear_selection(),
        Key::Escape if *s.selecting.peek() => s.selecting.set(false),
        Key::Escape if s.group.peek().is_some() => close_group(s),
        Key::Escape if *s.filter.peek() != Filter::All => show(s, Filter::All),
        Key::Delete if e.modifiers().contains(Modifiers::SHIFT) || s.in_trash_peek() => ask_delete(s, s.targets()),
        Key::Delete => queue::enqueue(s, Kind::Trash, s.targets()),
        Key::Character(c) if c == "/" => {
            e.prevent_default();
            if let Some(input) = s.search.peek().clone() {
                spawn(async move {
                    let _ = input.set_focus(true).await;
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(key: &str, n: usize) -> Group {
        Group {
            key: key.into(),
            org: key.into(),
            topic: String::new(),
            category: String::new(),
            ids: (0..n).map(|i| format!("{key}{i}")).collect(),
            sender: String::new(),
            subject: String::new(),
            first: String::new(),
            last: String::new(),
            summary: None,
        }
    }
    fn keys(groups: &[Group]) -> Vec<&str> {
        groups.iter().map(|g| g.key.as_str()).collect()
    }

    #[test]
    fn first_load_sorts_biggest_first() {
        let (order, shown, held) = merge(vec![], vec![group("a", 1), group("b", 5)], false, false);
        assert_eq!(order, ["b", "a"]);
        assert_eq!(keys(&shown).len(), 2);
        assert!(held.is_empty());
    }

    #[test]
    fn known_groups_keep_their_place_when_counts_change() {
        let order = vec!["a".to_string(), "b".to_string()];
        let (order, _, _) = merge(order, vec![group("a", 1), group("b", 50)], false, false);
        assert_eq!(order, ["a", "b"]);
    }

    #[test]
    fn new_groups_wait_while_syncing_then_append() {
        let order = vec!["a".to_string()];
        let (order2, shown, held) = merge(order.clone(), vec![group("a", 1), group("new", 9)], false, true);
        assert_eq!((order2, keys(&shown), keys(&held)), (vec!["a".to_string()], vec!["a"], vec!["new"]));
        let (order3, shown, held) = merge(order, vec![group("a", 1), group("new", 9)], false, false);
        assert_eq!((order3, keys(&shown), held.len()), (vec!["a".to_string(), "new".to_string()], vec!["a", "new"], 0));
    }

    #[test]
    fn vanished_groups_leave_the_order_and_resort_rebuilds_it() {
        let order = vec!["a".to_string(), "gone".to_string(), "b".to_string()];
        let (order2, _, _) = merge(order.clone(), vec![group("a", 1), group("b", 2)], false, false);
        assert_eq!(order2, ["a", "b"]);
        let (order3, _, _) = merge(order, vec![group("a", 1), group("b", 2)], true, false);
        assert_eq!(order3, ["b", "a"]);
    }
}
