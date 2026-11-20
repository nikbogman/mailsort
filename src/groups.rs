//! Emails grouped by (organization, topic), each with an AI summary and verdict.

use std::collections::HashMap;

use crate::db::{self, Email};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Verdict {
    Keep,
    Review,
    Delete,
}

impl Verdict {
    pub const ALL: [Verdict; 3] = [Verdict::Delete, Verdict::Review, Verdict::Keep];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "keep" => Some(Self::Keep),
            "review" => Some(Self::Review),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Review => "review",
            Self::Delete => "delete",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Keep => "Keep",
            Self::Review => "Review",
            Self::Delete => "Can go",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Summary {
    pub summary: String,
    pub verdict: Verdict,
    pub reason: String,
    /// The group's fingerprint when summarized; a different one means the group changed.
    pub fingerprint: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Group {
    pub key: String,
    pub org: String,
    pub topic: String,
    pub category: String,
    /// Newest first.
    pub ids: Vec<String>,
    /// A representative sender, for the avatar.
    pub sender: String,
    /// Subject of the newest email, shown until there's a summary.
    pub subject: String,
    pub first: String,
    pub last: String,
    pub summary: Option<Summary>,
}

impl Group {
    pub fn fingerprint(&self) -> String {
        format!("{}|{}", self.ids.len(), self.last)
    }

    pub fn is_stale(&self) -> bool {
        self.summary.as_ref().is_none_or(|s| s.fingerprint != self.fingerprint())
    }

    pub fn verdict(&self) -> Option<Verdict> {
        self.summary.as_ref().map(|s| s.verdict)
    }
}

pub fn key(org: &str, topic: &str) -> String {
    format!("{}|{}", org.trim().to_lowercase(), topic.trim().to_lowercase())
}

/// Groups in order of their newest email. `emails` must be newest first.
pub fn build(emails: &[Email], summaries: &HashMap<String, Summary>) -> Vec<Group> {
    let mut order: Vec<String> = vec![];
    let mut groups: HashMap<String, (Group, HashMap<&str, usize>)> = HashMap::new();
    for e in emails {
        let k = key(&e.org, &e.topic);
        let (g, categories) = groups.entry(k.clone()).or_insert_with(|| {
            order.push(k.clone());
            let group = Group {
                key: k.clone(),
                org: e.org.clone(),
                topic: e.topic.clone(),
                category: String::new(),
                ids: vec![],
                sender: e.sender.clone(),
                subject: e.subject.clone(),
                first: e.date.clone(),
                last: e.date.clone(),
                summary: summaries.get(&k).cloned(),
            };
            (group, HashMap::new())
        });
        g.ids.push(e.id.clone());
        g.first = e.date.clone();
        *categories.entry(&e.category).or_default() += 1;
    }
    order
        .into_iter()
        .filter_map(|k| groups.remove(&k))
        .map(|(mut g, categories)| {
            // The group's category is its most common one; ties go alphabetically so reloads agree.
            g.category = categories
                .into_iter()
                .max_by_key(|(c, n)| (*n, std::cmp::Reverse(*c)))
                .map(|(c, _)| c.to_string())
                .unwrap_or_default();
            g
        })
        .collect()
}

pub fn load() -> Vec<Group> {
    build(&db::analyzed(), &db::summaries())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn email(id: &str, org: &str, topic: &str, category: &str, date: &str) -> Email {
        Email {
            id: id.into(),
            sender: format!("{org} <x@{org}.com>"),
            subject: String::new(),
            date: date.into(),
            snippet: String::new(),
            category: category.into(),
            org: org.into(),
            topic: topic.into(),
        }
    }

    #[test]
    fn groups_by_org_and_topic_case_insensitively() {
        let emails = [
            email("3", "Vestide", "rent invoice", "Admin", "2026-11-01"),
            email("2", "NS", "travel update", "Notifications", "2026-10-01"),
            email("1", "vestide", "Rent Invoice", "Receipts", "2026-09-01"),
            email("0", "Vestide", "rent invoice", "Admin", "2026-08-01"),
        ];
        let groups = build(&emails, &HashMap::new());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].ids, ["3", "1", "0"]);
        assert_eq!((groups[0].first.as_str(), groups[0].last.as_str()), ("2026-08-01", "2026-11-01"));
        assert_eq!(groups[0].category, "Admin");
        assert!(groups[0].is_stale());
    }

    #[test]
    fn tied_category_is_stable_across_builds() {
        let emails = [email("2", "Bank", "x", "Receipts", "2026-11-01"), email("1", "Bank", "x", "Admin", "2026-10-01")];
        for _ in 0..50 {
            assert_eq!(build(&emails, &HashMap::new())[0].category, "Admin");
        }
    }

    #[test]
    fn summary_goes_stale_when_group_changes() {
        let emails = [email("1", "IMDb", "newsletter", "Promotions", "2026-11-01")];
        let mut summaries = HashMap::new();
        let summary = |fingerprint: &str| Summary { summary: "s".into(), verdict: Verdict::Delete, reason: "r".into(), fingerprint: fingerprint.into() };
        summaries.insert(key("IMDb", "newsletter"), summary("1|2026-11-01"));
        assert!(!build(&emails, &summaries)[0].is_stale());
        summaries.insert(key("IMDb", "newsletter"), summary("2|2026-11-01"));
        assert!(build(&emails, &summaries)[0].is_stale());
    }
}
