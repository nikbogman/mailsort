//! Local SQLite cache of emails, their analysis, and group summaries.
//! `gone` = no longer in the inbox (trashed, deleted, or removed in Gmail).

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use rusqlite::{Connection, Row, params, params_from_iter};

use crate::classify::{Analysis, Known};
use crate::groups::{Summary, Verdict};

#[derive(Clone, Debug, PartialEq)]
pub struct Email {
    pub id: String,
    pub sender: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
    pub category: String,
    /// Empty until analyzed.
    pub org: String,
    pub topic: String,
}

static DB: LazyLock<Mutex<Connection>> = LazyLock::new(|| {
    let conn = Connection::open(crate::config::db_path()).expect("open mail.db");
    conn.pragma_update(None, "journal_mode", "wal").expect("enable WAL");
    conn.busy_timeout(Duration::from_secs(5)).expect("set busy timeout");
    migrate(&conn).expect("migrate mail.db");
    Mutex::new(conn)
});

fn migrate(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch(
        "create table if not exists emails(id text primary key, sender text, subject text, date text,
           snippet text, category text, remove int, reason text, gone int default 0);
         create table if not exists groups(key text primary key, summary text, verdict text, reason text, fingerprint text);",
    )?;
    // Columns added after the first version (the Bun app's mail.db lacks them).
    let columns: HashSet<String> = c.prepare("select name from pragma_table_info('emails')")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    for column in ["org", "topic", "domain"] {
        if !columns.contains(column) {
            c.execute(&format!("alter table emails add column {column} text"), [])?;
        }
    }
    Ok(())
}

/// Runs `f` on the shared connection. A database error is a bug or a broken file, so it panics.
fn with<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> T {
    f(&DB.lock().unwrap()).expect("database error")
}

fn text(r: &Row, column: &str) -> rusqlite::Result<String> {
    Ok(r.get::<_, Option<String>>(column)?.unwrap_or_default())
}

fn email(r: &Row) -> rusqlite::Result<Email> {
    Ok(Email {
        id: r.get("id")?,
        sender: text(r, "sender")?,
        subject: text(r, "subject")?,
        date: text(r, "date")?,
        snippet: text(r, "snippet")?,
        category: text(r, "category")?,
        org: text(r, "org")?,
        topic: text(r, "topic")?,
    })
}

fn emails(sql: &str, args: impl rusqlite::Params) -> Vec<Email> {
    with(|c| c.prepare(sql)?.query_map(args, email)?.collect())
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// Inbox emails that have been analyzed, newest first.
pub fn analyzed() -> Vec<Email> {
    emails("select * from emails where gone=0 and coalesce(org,'') != '' order by date desc", [])
}

/// Inbox emails still waiting for analysis, newest first.
pub fn unanalyzed() -> Vec<Email> {
    emails("select * from emails where gone=0 and coalesce(org,'') = '' order by date desc", [])
}

pub fn unanalyzed_count() -> usize {
    with(|c| c.query_row("select count(*) from emails where gone=0 and coalesce(org,'') = ''", [], |r| r.get::<_, i64>(0))) as usize
}

pub fn by_ids(ids: &[String]) -> Vec<Email> {
    let sql = format!("select * from emails where id in ({}) order by date desc", placeholders(ids.len()));
    emails(&sql, params_from_iter(ids))
}

pub fn known_ids() -> HashSet<String> {
    with(|c| c.prepare("select id from emails")?.query_map([], |r| r.get(0))?.collect())
}

/// Ignores emails already stored (another sync may have saved it).
pub fn insert(e: &Email, gone: bool) {
    with(|c| {
        c.execute(
            "insert or ignore into emails(id, sender, subject, date, snippet, category, remove, reason, gone, org, topic)
             values(?,?,?,?,?,?,0,'',?,?,?)",
            params![e.id, e.sender, e.subject, e.date, e.snippet, e.category, gone, e.org, e.topic],
        )
    });
}

pub fn set_analysis(id: &str, a: &Analysis, domain: &str) {
    with(|c| {
        c.execute(
            "update emails set category=?, org=?, topic=?, domain=? where id=?",
            params![a.category, a.org, a.topic, domain, id],
        )
    });
}

/// Organizations and topics already used for a sender domain, so new emails reuse them.
pub fn known_for_domain(domain: &str) -> Known {
    let distinct = |column: &str| -> Vec<String> {
        let sql = format!(
            "select {column} from emails where domain=? and coalesce({column},'') != '' group by {column} order by count(*) desc limit 15"
        );
        with(|c| c.prepare(&sql)?.query_map([domain], |r| r.get(0))?.collect())
    };
    Known { orgs: distinct("org"), topics: distinct("topic") }
}

pub fn summaries() -> HashMap<String, Summary> {
    with(|c| {
        c.prepare("select key, summary, verdict, reason, fingerprint from groups")?
            .query_map([], |r| {
                let verdict: String = r.get(2)?;
                Ok((
                    r.get(0)?,
                    Summary {
                        summary: r.get(1)?,
                        verdict: Verdict::parse(&verdict).unwrap_or(Verdict::Review),
                        reason: r.get(3)?,
                        fingerprint: r.get(4)?,
                    },
                ))
            })?
            .collect()
    })
}

pub fn save_summary(key: &str, s: &Summary) {
    with(|c| {
        c.execute(
            "insert or replace into groups(key, summary, verdict, reason, fingerprint) values(?,?,?,?,?)",
            params![key, s.summary, s.verdict.as_str(), s.reason, s.fingerprint],
        )
    });
}

pub fn set_gone(id: &str, gone: bool) {
    with(|c| c.execute("update emails set gone=? where id=?", params![gone, id]));
}

/// After seeing the whole inbox: anything we know that's not in it was removed elsewhere.
pub fn mark_gone_except(inbox: &[String]) {
    let sql = format!("update emails set gone=1 where id not in ({})", placeholders(inbox.len()));
    with(|c| c.execute(&sql, params_from_iter(inbox)));
}
