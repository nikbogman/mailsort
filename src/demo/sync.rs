//! Web demo stand-in for sync: plays the three stages with fake progress, finds nothing new.

use std::cell::Cell;
use std::time::Duration;

use crate::db::{self, Email};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stage {
    Fetching,
    Analyzing,
    Summarizing,
}

pub const STOPPED: &str = "stopped";

thread_local! {
    static STOP: Cell<bool> = const { Cell::new(false) };
}

pub fn stop() {
    STOP.set(true);
}

pub async fn sync(mut progress: impl FnMut(Stage, usize, usize)) -> Result<usize, String> {
    STOP.set(false);
    for (stage, total) in [(Stage::Fetching, 24), (Stage::Analyzing, 24), (Stage::Summarizing, 6)] {
        for done in 1..=total {
            crate::sleep(Duration::from_millis(90)).await;
            if STOP.get() {
                return Err(STOPPED.into());
            }
            progress(stage, done, total);
        }
    }
    Ok(0)
}

pub async fn trash_emails() -> Result<Vec<Email>, String> {
    crate::sleep(Duration::from_millis(300)).await;
    Ok(db::trashed())
}
