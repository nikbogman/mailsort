//! Delete/trash/restore queue: emails leave the list instantly, Gmail calls run in the background
//! one job at a time, in chunks, so the UI can show progress and offer Undo/Stop.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use web_time::Instant;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::db;
use crate::google;
use crate::ui::{State, reload};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Trash,
    Restore,
    Delete,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum JobState {
    Queued,
    Running,
    Done,
    Undoing,
    Undone,
    Stopped,
    Error,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Job {
    pub id: u64,
    pub kind: Kind,
    pub ids: Vec<String>,
    pub done: usize,
    pub failed: Vec<String>,
    pub state: JobState,
    pub created: Instant,
}

impl Job {
    pub fn is_pending(&self) -> bool {
        matches!(self.state, JobState::Queued | JobState::Running)
    }

    fn succeeded(&self) -> Vec<String> {
        self.ids[..self.done].iter().filter(|id| !self.failed.contains(id)).cloned().collect()
    }
}

/// Time to undo a permanent delete before it starts.
pub const DELETE_GRACE: Duration = Duration::from_secs(5);
const HIDE_AFTER: Duration = Duration::from_secs(8);

static WORKING: AtomicBool = AtomicBool::new(false);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Emails per step, so progress moves. Trash/restore is one Gmail call per email, delete is one batch call.
fn chunk_size(kind: Kind) -> usize {
    match kind {
        Kind::Delete => 100,
        Kind::Trash | Kind::Restore => 10,
    }
}

/// Remove from the list right away, then queue the Gmail calls.
pub fn enqueue(mut s: State, kind: Kind, ids: Vec<String>) {
    if ids.is_empty() {
        return;
    }
    s.forget(&ids.iter().cloned().collect::<HashSet<String>>());
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    s.jobs.write().push(Job { id, kind, ids, done: 0, failed: vec![], state: JobState::Queued, created: Instant::now() });
    kick(s);
}

fn kick(s: State) {
    if !WORKING.swap(true, Ordering::SeqCst) {
        spawn_forever(work(s));
    }
}

fn job(s: State, id: u64) -> Option<Job> {
    s.jobs.peek().iter().find(|j| j.id == id).cloned()
}

fn update(mut s: State, id: u64, f: impl FnOnce(&mut Job)) {
    if let Some(j) = s.jobs.write().iter_mut().find(|j| j.id == id) {
        f(j);
    }
}

fn state_of(s: State, id: u64) -> Option<JobState> {
    job(s, id).map(|j| j.state)
}

fn next_queued(s: State) -> Option<u64> {
    s.jobs.peek().iter().find(|j| j.state == JobState::Queued).map(|j| j.id)
}

async fn work(s: State) {
    while let Some(id) = next_queued(s) {
        let Some(j) = job(s, id) else { continue };
        if j.kind == Kind::Delete {
            crate::sleep((j.created + DELETE_GRACE).saturating_duration_since(Instant::now())).await;
            if state_of(s, id) != Some(JobState::Queued) {
                continue; // undone during the grace period
            }
        }
        update(s, id, |j| j.state = JobState::Running);
        for chunk in j.ids.chunks(chunk_size(j.kind)) {
            if state_of(s, id) != Some(JobState::Running) {
                break; // undo or stop: finish the current chunk, skip the rest
            }
            let failed = run_chunk(s, j.kind, chunk).await;
            update(s, id, |j| {
                j.done += chunk.len();
                j.failed.extend(failed);
            });
        }
        match job(s, id) {
            Some(j) if j.state == JobState::Running => {
                finish(s, id, if j.failed.is_empty() { JobState::Done } else { JobState::Error })
            }
            Some(j) if j.state == JobState::Undoing => {
                restore(s, j.succeeded()).await;
                finish(s, id, JobState::Undone);
            }
            Some(j) if j.state == JobState::Stopped => finish(s, id, JobState::Stopped),
            _ => {}
        }
    }
    WORKING.store(false, Ordering::SeqCst);
    if next_queued(s).is_some() {
        kick(s); // queued while we were finishing up
    }
    reload(s).await; // resync counts with the database
}

/// Returns the ids that failed; the first error is shown in the UI.
async fn run_chunk(mut s: State, kind: Kind, chunk: &[String]) -> Vec<String> {
    let results: Vec<(String, google::Result<()>)> = match kind {
        Kind::Delete => {
            let r = google::delete_forever(chunk).await;
            chunk.iter().map(|id| (id.clone(), r.clone())).collect()
        }
        Kind::Trash | Kind::Restore => {
            futures::future::join_all(chunk.iter().map(|id| async move {
                let r = if kind == Kind::Trash { google::trash(id).await } else { google::untrash(id).await };
                (id.clone(), r)
            }))
            .await
        }
    };
    let mut failed = vec![];
    for (id, r) in results {
        match r {
            Ok(()) => db::set_gone(&id, kind != Kind::Restore),
            Err(e) => {
                if s.error.peek().is_none() {
                    s.error.set(Some(e));
                }
                failed.push(id);
            }
        }
    }
    failed
}

async fn restore(s: State, ids: Vec<String>) {
    for chunk in ids.chunks(chunk_size(Kind::Restore)) {
        run_chunk(s, Kind::Restore, chunk).await;
    }
}

fn finish(s: State, id: u64, state: JobState) {
    update(s, id, |j| j.state = state);
    let mut jobs = s.jobs;
    spawn_forever(async move {
        crate::sleep(HIDE_AFTER).await;
        jobs.write().retain(|j| j.id != id);
    });
}

/// Cancel a queued job; for trash jobs also restore what was already trashed.
pub fn undo(s: State, id: u64) {
    let Some(j) = job(s, id) else { return };
    match (j.kind, j.state) {
        (_, JobState::Queued) => {
            finish(s, id, JobState::Undone);
            spawn_forever(reload(s));
        }
        (Kind::Trash, JobState::Running) => update(s, id, |j| j.state = JobState::Undoing), // the worker restores
        (Kind::Trash, JobState::Done | JobState::Error) => {
            update(s, id, |j| j.state = JobState::Undoing);
            spawn_forever(async move {
                restore(s, j.succeeded()).await;
                finish(s, id, JobState::Undone);
                reload(s).await;
            });
        }
        _ => {}
    }
}

/// Stop a running permanent delete. What's already deleted stays deleted.
pub fn stop(s: State, id: u64) {
    if job(s, id).is_some_and(|j| j.kind == Kind::Delete && j.state == JobState::Running) {
        update(s, id, |j| j.state = JobState::Stopped);
    }
}
