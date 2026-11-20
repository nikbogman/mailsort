//! Floating UI: the delete queue cards and the "Delete forever?" dialog.

use dioxus::prelude::*;

use super::State;
use crate::icons::icon;
use crate::queue::{self, DELETE_GRACE, Job, JobState, Kind};

#[component]
pub fn ErrorBanner() -> Element {
    let mut s = use_context::<State>();
    let Some(error) = s.error.read().clone() else { return rsx! {} };
    rsx! {
        div { class: "banner",
            span { "{error}" }
            button { class: "icon", title: "Dismiss", onclick: move |_| s.error.set(None), {icon("x")} }
        }
    }
}

#[component]
pub fn Queue() -> Element {
    let s = use_context::<State>();
    let jobs = s.jobs.read().clone();
    rsx! {
        div { class: "queue",
            for j in jobs {
                QueueCard { key: "{j.id}", job: j.clone() }
            }
        }
    }
}

fn label(job: &Job) -> String {
    let n = job.ids.len();
    let ok = job.done - job.failed.len();
    match (job.kind, job.state) {
        (Kind::Trash, JobState::Queued) => format!("Waiting to trash {n}"),
        (Kind::Trash, JobState::Running) => format!("Trashing {n} · {}/{n}", job.done),
        (Kind::Trash, JobState::Done) => format!("Moved {n} to Trash"),
        (Kind::Restore, JobState::Queued) => format!("Waiting to restore {n}"),
        (Kind::Restore, JobState::Running) => format!("Restoring {n} · {}/{n}", job.done),
        (Kind::Restore, JobState::Done) => format!("Restored {n} to inbox"),
        (Kind::Delete, JobState::Queued) => format!("Deleting {n} forever…"),
        (Kind::Delete, JobState::Running) => format!("Deleting {n} · {}/{n}", job.done),
        (Kind::Delete, JobState::Done) => format!("Deleted {n} forever"),
        (Kind::Delete, JobState::Undone) => "Delete cancelled".into(),
        (_, JobState::Undoing) => "Restoring…".into(),
        (_, JobState::Undone) => format!("Restored {n}"),
        (_, JobState::Stopped) => format!("Stopped · {ok} of {n} deleted"),
        (_, JobState::Error) => format!("{} of {n} failed", job.failed.len()),
    }
}

#[component]
fn QueueCard(job: Job) -> Element {
    let s = use_context::<State>();
    let can_undo = match job.kind {
        Kind::Trash => matches!(job.state, JobState::Queued | JobState::Running | JobState::Done | JobState::Error),
        Kind::Delete => job.state == JobState::Queued,
        Kind::Restore => false,
    };
    let can_stop = job.kind == Kind::Delete && job.state == JobState::Running;
    let busy = matches!(job.state, JobState::Queued | JobState::Running | JobState::Undoing);
    let kind = format!("{:?}", job.kind).to_lowercase();
    let state = format!("{:?}", job.state).to_lowercase();
    // Grace countdown: the bar drains over DELETE_GRACE, resumed at the right point after re-renders.
    let countdown = job.kind == Kind::Delete && job.state == JobState::Queued;
    let elapsed = job.created.elapsed().as_millis();
    let grace = DELETE_GRACE.as_millis();
    let percent = if job.ids.is_empty() { 0 } else { job.done * 100 / job.ids.len() };
    let id = job.id;

    rsx! {
        div { class: "job {kind} {state}",
            div { class: "job-row",
                if busy {
                    span { class: "spinner" }
                } else if job.state == JobState::Done {
                    {icon("check")}
                } else if job.state == JobState::Error {
                    {icon("x")}
                }
                span { class: "status", "{label(&job)}" }
                if can_undo {
                    button { onclick: move |_| queue::undo(s, id), "Undo" }
                } else if can_stop {
                    button { onclick: move |_| queue::stop(s, id), "Stop" }
                }
            }
            div { class: "bar",
                if countdown {
                    span { class: "countdown", style: "animation-duration:{grace}ms;animation-delay:-{elapsed}ms" }
                } else {
                    span { style: "width:{percent}%" }
                }
            }
        }
    }
}

#[component]
pub fn ConfirmDialog() -> Element {
    let mut s = use_context::<State>();
    let Some(ids) = s.confirm.read().clone() else { return rsx! {} };
    let n = ids.len();
    let what = if n == 1 { "1 email".to_string() } else { format!("{n} emails") };
    let detail = if s.in_trash() { "This can't be undone." } else { "This skips the Trash and can't be undone." };
    rsx! {
        div { class: "backdrop", onclick: move |_| s.confirm.set(None),
            div { class: "dialog", onclick: move |e| e.stop_propagation(),
                div { class: "dialog-icon", {icon("deleteForever")} }
                h3 { "Delete forever?" }
                p { "{what} will be permanently deleted. {detail}" }
                div { class: "dialog-actions",
                    button {
                        class: "btn",
                        onmounted: move |e| async move {
                            let _ = e.set_focus(true).await; // safe default
                        },
                        onclick: move |_| s.confirm.set(None),
                        "Cancel"
                    }
                    button {
                        class: "btn danger",
                        onclick: move |_| {
                            s.confirm.set(None);
                            queue::enqueue(s, Kind::Delete, ids.clone());
                        },
                        "Delete {what}"
                    }
                }
            }
        }
    }
}
