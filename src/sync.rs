//! Sync in three stages: fetch new inbox mail, analyze unanalyzed emails, summarize changed groups.

use std::sync::atomic::{AtomicBool, Ordering};

use futures::StreamExt;

use crate::classify::{Input, analyze, ensure_ollama, sender_domain, sender_name, summarize};
use crate::config::LIMIT;
use crate::db::{self, Email};
use crate::google;
use crate::groups::{self, Summary};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stage {
    Fetching,
    Analyzing,
    Summarizing,
}

/// Error returned when the user pressed Stop. Work done so far is kept.
pub const STOPPED: &str = "stopped";
static STOP: AtomicBool = AtomicBool::new(false);

/// Ask a running sync to stop after the current email.
pub fn stop() {
    STOP.store(true, Ordering::SeqCst);
}

fn check_stop() -> Result<(), String> {
    if STOP.load(Ordering::SeqCst) { Err(STOPPED.into()) } else { Ok(()) }
}

/// Reports (stage, done, total). Returns how many new emails arrived.
/// A failed fetch doesn't stop analysis and summaries of what's already stored; its error is returned at the end.
pub async fn sync(mut progress: impl FnMut(Stage, usize, usize)) -> Result<usize, String> {
    STOP.store(false, Ordering::SeqCst);
    let fetched = fetch(&mut progress).await;
    check_stop()?;
    analyze_and_summarize(&mut progress).await?;
    fetched
}

/// 1. Fetch headers of new inbox mail, a few at a time (Gmail rate-limits bursts).
async fn fetch(progress: &mut impl FnMut(Stage, usize, usize)) -> Result<usize, String> {
    let (ids, complete) = google::list_messages("INBOX", LIMIT).await?;
    if complete {
        db::mark_gone_except(&ids); // known emails missing from the full inbox were removed elsewhere
    }
    let known = db::known_ids();
    let fresh: Vec<String> = ids.into_iter().filter(|id| !known.contains(id)).collect();
    let mut headers = futures::stream::iter(fresh.iter().cloned())
        .map(|id| async move { (id.clone(), google::get_headers(&id).await) })
        .buffer_unordered(5);
    let mut fetched = 0;
    while let Some((id, h)) = headers.next().await {
        check_stop()?;
        let h = h?;
        let email = Email { id, sender: h.sender, subject: h.subject, date: h.date, snippet: h.snippet, category: String::new(), org: String::new(), topic: String::new() };
        db::insert(&email, false);
        fetched += 1;
        progress(Stage::Fetching, fetched, fresh.len());
    }
    Ok(fresh.len())
}

async fn analyze_and_summarize(progress: &mut impl FnMut(Stage, usize, usize)) -> Result<(), String> {
    // 2. Analyze: category, organization, topic. Also catches up emails from older versions.
    let todo = db::unanalyzed();
    if !todo.is_empty() {
        ensure_ollama().await?;
    }
    for (i, e) in todo.iter().enumerate() {
        check_stop()?;
        let domain = sender_domain(&e.sender);
        let known = db::known_for_domain(&domain);
        let input = Input { sender: &e.sender, subject: &e.subject, snippet: &e.snippet };
        let analysis = analyze(&input, &known, &sender_name(&e.sender)).await?;
        db::set_analysis(&e.id, &analysis, &domain);
        progress(Stage::Analyzing, i + 1, todo.len());
    }

    // 3. Summarize new or changed groups, biggest first so the overview fills in usefully.
    let mut stale: Vec<groups::Group> = groups::load().into_iter().filter(|g| g.is_stale()).collect();
    stale.sort_by_key(|g| std::cmp::Reverse(g.ids.len()));
    if !stale.is_empty() {
        ensure_ollama().await?;
    }
    for (i, g) in stale.iter().enumerate() {
        check_stop()?;
        let subjects: Vec<String> = db::by_ids(&g.ids[..g.ids.len().min(12)]).into_iter().map(|e| e.subject).collect();
        let (summary, verdict, reason) = summarize(g, &subjects).await?;
        db::save_summary(&g.key, &Summary { summary, verdict, reason, fingerprint: g.fingerprint() });
        progress(Stage::Summarizing, i + 1, stale.len());
    }
    Ok(())
}

/// What's in Gmail's Trash right now. Emails we haven't seen (trashed in Gmail itself) get their headers fetched once.
pub async fn trash_emails() -> Result<Vec<Email>, String> {
    let (ids, _) = google::list_messages("TRASH", LIMIT).await?;
    let known = db::known_ids();
    let unknown: Vec<String> = ids.iter().filter(|id| !known.contains(*id)).cloned().collect();
    futures::stream::iter(unknown)
        .for_each_concurrent(5, |id| async move {
            if let Ok(h) = google::get_headers(&id).await {
                let email = Email { id, sender: h.sender, subject: h.subject, date: h.date, snippet: h.snippet, category: String::new(), org: String::new(), topic: String::new() };
                db::insert(&email, true);
            }
        })
        .await;
    Ok(db::by_ids(&ids))
}
