//! Web demo stand-in for the SQLite cache: a made-up inbox kept in memory, reset on page load.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::groups::{Summary, Verdict};

#[derive(Clone, Debug, PartialEq)]
pub struct Email {
    pub id: String,
    pub sender: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
    pub category: String,
    pub org: String,
    pub topic: String,
}

struct Seed {
    sender: &'static str,
    org: &'static str,
    topic: &'static str,
    category: &'static str,
    count: usize,
    /// Hours between emails, newest one `every` hours ago.
    every: i64,
    verdict: Verdict,
    summary: &'static str,
    reason: &'static str,
    subjects: &'static [&'static str],
}

use Verdict::{Delete, Keep, Review};

#[rustfmt::skip]
const SEEDS: &[Seed] = &[
    Seed { sender: "Medium Daily Digest <noreply@medium.com>", org: "Medium", topic: "daily digest", category: "Newsletters", count: 96, every: 22, verdict: Delete, summary: "Daily picks of stories based on what you read.", reason: "Old digests, nothing to act on", subjects: &["Why Rust is eating the backend", "The quiet death of the side project", "10 CSS tricks you missed", "Designing for calm"] },
    Seed { sender: "Substack <no-reply@substack.com>", org: "Substack", topic: "newsletter", category: "Newsletters", count: 58, every: 30, verdict: Delete, summary: "Weekly posts from writers you follow.", reason: "Readable on the web any time", subjects: &["Notes on taste", "The weekly roundup", "What I learned shipping alone"] },
    Seed { sender: "The Verge <newsletter@theverge.com>", org: "The Verge", topic: "newsletter", category: "Newsletters", count: 34, every: 48, verdict: Review, summary: "Tech news in your inbox each morning.", reason: "A few may still be unread", subjects: &["Verge Daily: the week in phones", "Installer: apps worth a look"] },
    Seed { sender: "GitHub <notifications@github.com>", org: "GitHub", topic: "ci failure", category: "Dev Tools", count: 72, every: 9, verdict: Delete, summary: "Failed workflow runs on your repositories.", reason: "All runs were fixed later", subjects: &["[mailsort] Run failed: build (main)", "[dotfiles] Run failed: lint", "[mailsort] Run failed: test (pr #41)"] },
    Seed { sender: "GitHub <notifications@github.com>", org: "GitHub", topic: "pull request", category: "Dev Tools", count: 41, every: 20, verdict: Keep, summary: "Reviews and comments on your pull requests.", reason: "Threads you may want to look back at", subjects: &["Re: [mailsort] Treemap layout (#38)", "[metal] Approved: wake on LAN script (#12)"] },
    Seed { sender: "npm <support@npmjs.com>", org: "npm", topic: "security advisory", category: "Dev Tools", count: 18, every: 60, verdict: Delete, summary: "Advisories for packages you published.", reason: "Patched long ago", subjects: &["Security advisory for a dependency", "New version published"] },
    Seed { sender: "Zalando <info@zalando.com>", org: "Zalando", topic: "promotion", category: "Promotions", count: 64, every: 26, verdict: Delete, summary: "Sales and new arrivals.", reason: "Expired offers", subjects: &["Up to 50% off, this weekend only", "New in: winter jackets", "Your size is back"] },
    Seed { sender: "Bol <nieuwsbrief@bol.com>", org: "Bol", topic: "promotion", category: "Promotions", count: 52, every: 30, verdict: Delete, summary: "Deals and product picks.", reason: "Old marketing, no value", subjects: &["Black Friday starts now", "Picked for you"] },
    Seed { sender: "Steam <noreply@steampowered.com>", org: "Steam", topic: "wishlist sale", category: "Promotions", count: 40, every: 40, verdict: Delete, summary: "Games on your wishlist went on sale.", reason: "Sales have ended", subjects: &["An item on your wishlist is on sale!", "Autumn Sale is live"] },
    Seed { sender: "Prize Desk <winner@lucky-prize.biz>", org: "Prize Desk", topic: "prize claim", category: "Spam", count: 38, every: 34, verdict: Delete, summary: "Messages claiming you won something.", reason: "Junk and likely phishing", subjects: &["You have (1) unclaimed reward", "Final notice: claim your prize"] },
    Seed { sender: "LinkedIn <notifications@linkedin.com>", org: "LinkedIn", topic: "notification", category: "Social", count: 54, every: 18, verdict: Delete, summary: "Profile views, likes and suggested connections.", reason: "Social notifications age fast", subjects: &["You appeared in 12 searches this week", "Anna commented on your post"] },
    Seed { sender: "Discord <noreply@discord.com>", org: "Discord", topic: "mentions", category: "Social", count: 26, every: 36, verdict: Delete, summary: "Mentions and messages you missed.", reason: "Already seen in the app", subjects: &["You have 4 unread messages", "New mention in #rust"] },
    Seed { sender: "PostNL <noreply@postnl.nl>", org: "PostNL", topic: "delivery update", category: "Notifications", count: 30, every: 44, verdict: Delete, summary: "Tracking updates for parcels.", reason: "Parcels were delivered", subjects: &["Your parcel is on its way", "Delivered: parcel from Bol"] },
    Seed { sender: "Google Calendar <calendar-notification@google.com>", org: "Google Calendar", topic: "reminder", category: "Notifications", count: 22, every: 50, verdict: Delete, summary: "Reminders for past events.", reason: "Events are over", subjects: &["Reminder: dentist @ Thu 10:00", "Reminder: team sync"] },
    Seed { sender: "Sara de Vries <sara@studio-noord.nl>", org: "Studio Noord", topic: "project", category: "Work", count: 21, every: 70, verdict: Review, summary: "Sara on the website project: drafts and feedback.", reason: "Last mail asks for a reply", subjects: &["Re: homepage draft v3", "Invoice question", "Can we move Friday?"] },
    Seed { sender: "Tom Bakker <tom@acme.io>", org: "Acme", topic: "meeting", category: "Work", count: 14, every: 90, verdict: Keep, summary: "Meeting notes and follow-ups with Tom.", reason: "Written by a person; worth keeping", subjects: &["Notes from Tuesday", "Follow-up on the proposal"] },
    Seed { sender: "Google <no-reply@accounts.google.com>", org: "Google", topic: "security alert", category: "Security", count: 19, every: 80, verdict: Delete, summary: "New sign-in alerts for your account.", reason: "Old sign-ins, all were you", subjects: &["Security alert: new sign-in on Windows", "Your 2-Step Verification code"] },
    Seed { sender: "Microsoft <account-security@microsoft.com>", org: "Microsoft", topic: "security code", category: "Security", count: 12, every: 110, verdict: Delete, summary: "One-time codes for signing in.", reason: "Codes expire in minutes", subjects: &["Your single-use code", "Unusual sign-in activity"] },
    Seed { sender: "Grafana <alerts@grafana.net>", org: "Grafana", topic: "alert", category: "Alerts", count: 22, every: 28, verdict: Delete, summary: "Firing and resolved alerts from the home server.", reason: "All resolved", subjects: &["[FIRING] disk usage > 90% on blackberry", "[RESOLVED] disk usage"] },
    Seed { sender: "Mom <mom@hey.com>", org: "Mom", topic: "family", category: "Personal", count: 16, every: 120, verdict: Keep, summary: "Photos and plans from home.", reason: "Family mail", subjects: &["Photos from Sunday", "Christmas plans?"] },
    Seed { sender: "Vestide <huur@vestide.nl>", org: "Vestide", topic: "rent invoice", category: "Receipts", count: 24, every: 720, verdict: Keep, summary: "Monthly rent invoices and payment confirmations.", reason: "Needed for tax records", subjects: &["Your rent invoice", "Payment received"] },
    Seed { sender: "Amazon <auto-confirm@amazon.nl>", org: "Amazon", topic: "order confirmation", category: "Receipts", count: 24, every: 200, verdict: Keep, summary: "Order confirmations and invoices.", reason: "Proof of purchase for warranty", subjects: &["Your order has been placed", "Your invoice"] },
    Seed { sender: "Vercel <notifications@vercel.com>", org: "Vercel", topic: "usage update", category: "Services", count: 21, every: 96, verdict: Delete, summary: "Usage reports and plan notices.", reason: "Routine automated notices", subjects: &["Your monthly usage report", "Updates to our terms"] },
    Seed { sender: "Railway <team@railway.app>", org: "Railway", topic: "account update", category: "Services", count: 18, every: 120, verdict: Review, summary: "Billing and plan changes.", reason: "Card expires soon", subjects: &["Your card is about to expire", "Plan changes coming"] },
    Seed { sender: "Belastingdienst <noreply@belastingdienst.nl>", org: "Belastingdienst", topic: "tax", category: "Admin", count: 18, every: 400, verdict: Keep, summary: "Tax returns and assessments.", reason: "Keep tax letters for 7 years", subjects: &["New message in your inbox", "Assessment 2025"] },
    Seed { sender: "RustConf <hello@rustconf.com>", org: "RustConf", topic: "event promotion", category: "Events", count: 14, every: 140, verdict: Delete, summary: "Talks, tickets and speaker news.", reason: "The event is over", subjects: &["Early bird tickets end Friday", "The schedule is live"] },
];

#[rustfmt::skip]
const SNIPPETS: &[&str] = &[
    "Here's what's new since last time, picked for you.",
    "View this email in your browser if it doesn't look right.",
    "Thanks for being with us. A quick update on your account.",
    "Open the app to see the full details.",
];

struct Mail {
    email: Email,
    gone: bool,
}

fn seed() -> Vec<Mail> {
    let now = chrono::Local::now();
    let date = |hours: i64| (now - chrono::Duration::hours(hours)).format("%Y-%m-%d %H:%M").to_string();
    let mut mails = vec![];
    for (g, s) in SEEDS.iter().enumerate() {
        for i in 0..s.count {
            let hours = s.every * (i as i64 + 1) - (g as i64 % 5);
            mails.push(Mail {
                email: Email {
                    id: format!("{g}-{i}"),
                    sender: s.sender.into(),
                    subject: s.subjects[i % s.subjects.len()].into(),
                    date: date(hours),
                    snippet: SNIPPETS[(g + i) % SNIPPETS.len()].into(),
                    category: s.category.into(),
                    org: s.org.into(),
                    topic: s.topic.into(),
                },
                // A few of each newsletter are already in Trash.
                gone: s.category == "Newsletters" && i % 12 == 5,
            });
        }
    }
    mails.sort_by(|a, b| b.email.date.cmp(&a.email.date));
    mails
}

thread_local! {
    static MAIL: RefCell<Vec<Mail>> = RefCell::new(seed());
}

fn select(f: impl Fn(&Mail) -> bool) -> Vec<Email> {
    MAIL.with_borrow(|m| m.iter().filter(|m| f(m)).map(|m| m.email.clone()).collect())
}

pub fn analyzed() -> Vec<Email> {
    select(|m| !m.gone)
}

pub fn unanalyzed_count() -> usize {
    0
}

pub fn by_ids(ids: &[String]) -> Vec<Email> {
    select(|m| ids.contains(&m.email.id))
}

pub fn trashed() -> Vec<Email> {
    select(|m| m.gone)
}

pub fn summaries() -> HashMap<String, Summary> {
    SEEDS
        .iter()
        .map(|s| {
            let summary = Summary { summary: s.summary.into(), verdict: s.verdict, reason: s.reason.into(), fingerprint: String::new() };
            (crate::groups::key(s.org, s.topic), summary)
        })
        .collect()
}

pub fn set_gone(id: &str, gone: bool) {
    MAIL.with_borrow_mut(|m| m.iter_mut().filter(|m| m.email.id == id).for_each(|m| m.gone = gone));
}

pub fn remove(ids: &[String]) {
    MAIL.with_borrow_mut(|m| m.retain(|m| !ids.contains(&m.email.id)));
}
