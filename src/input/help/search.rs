use super::{HelpEntry, HelpSection};
use crate::input::Action;

/// A jump always addresses a row in the original document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpTarget {
    Section(HelpSection),
    Action(Action),
}

/// Ranked picker row with effective shortcuts and a category subtitle.
#[derive(Clone, Debug)]
pub struct HelpHit {
    pub target: HelpTarget,
    pub title: String,
    pub keys: String,
    pub section: HelpSection,
}

/// Search text and selected result belong only to the current Help session.
#[derive(Clone, Debug, Default)]
pub struct HelpPicker {
    pub query: String,
    pub selected: usize,
}

/// Exact/title matches precede shortcut and fuzzy description matches; ties retain document order.
pub fn help_hits(entries: &[HelpEntry], query: &str) -> Vec<HelpHit> {
    let query = query.trim().to_lowercase();
    let mut hits = Vec::new();
    for section in HelpSection::ALL {
        let title = section.title();
        if let Some(rank) = rank(&query, title, "", title) {
            hits.push((
                rank,
                HelpHit {
                    target: HelpTarget::Section(section),
                    title: title.into(),
                    keys: String::new(),
                    section,
                },
            ));
        }
        if query.is_empty() {
            continue;
        }
        for entry in entries
            .iter()
            .filter(|entry| entry.action.help_section() == section)
        {
            let action = format!("{:?}", entry.action).to_lowercase();
            let extra = format!("{action} {} {title}", entry.description);
            if let Some(mut score) = rank(&query, entry.description, &entry.keys, &extra) {
                if action == query.replace(' ', "") {
                    score = 0;
                }
                hits.push((
                    score,
                    HelpHit {
                        target: HelpTarget::Action(entry.action),
                        title: entry.description.into(),
                        keys: entry.keys.clone(),
                        section,
                    },
                ));
            }
        }
    }
    hits.sort_by_key(|(score, _)| *score);
    hits.into_iter().map(|(_, hit)| hit).collect()
}

fn rank(query: &str, title: &str, keys: &str, extra: &str) -> Option<u8> {
    let title = title.to_lowercase();
    let keys = keys.to_lowercase();
    if query.is_empty() || title == query {
        Some(0)
    } else if title.starts_with(query) {
        Some(1)
    } else if title.contains(query) {
        Some(2)
    } else if !keys.is_empty() && keys.contains(query) {
        Some(3)
    } else if subsequence(&title, query) {
        Some(4)
    } else if subsequence(&extra.to_lowercase(), query) {
        Some(5)
    } else {
        None
    }
}

fn subsequence(text: &str, query: &str) -> bool {
    let mut chars = text.chars();
    query
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|needle| chars.any(|c| c == needle))
}
