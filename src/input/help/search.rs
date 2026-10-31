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
        if let Some(rank) = rank(&query, title, "", "") {
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
            if let Some(score) = rank(&query, entry.description, &entry.keys, &action) {
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

// Fields never share a fuzzy span; category text cannot match unrelated commands.
fn rank(query: &str, title: &str, keys: &str, action: &str) -> Option<(u8, usize)> {
    let title = text_rank(query, &title.to_lowercase());
    let action = text_rank(&query.replace(' ', ""), action);
    let keys = (!keys.is_empty() && keys.to_lowercase().contains(query)).then_some((3, 0));
    [title, action, keys].into_iter().flatten().min()
}

fn text_rank(query: &str, text: &str) -> Option<(u8, usize)> {
    if query.is_empty() || text == query {
        return Some((0, 0));
    }
    if text.starts_with(query) {
        return Some((1, 0));
    }
    if text.contains(query) || query.split_whitespace().all(|word| text.contains(word)) {
        return Some((2, 0));
    }
    if query.chars().count() < 3 {
        return None;
    }
    text.split(|c: char| !c.is_alphanumeric())
        .filter_map(|word| fuzzy_gap(word, query))
        .min()
        .map(|gap| (4, gap))
}

// Abbreviations start at a word boundary and keep at least half the spanned letters.
fn fuzzy_gap(word: &str, query: &str) -> Option<usize> {
    if word.chars().next()? != query.chars().next()? {
        return None;
    }
    let mut letters = word.chars();
    let mut span = 0;
    let count = query.chars().count();
    for needle in query.chars() {
        let skipped = letters.by_ref().position(|c| c == needle)?;
        span += skipped + 1;
    }
    (span <= count * 2).then_some(span - count)
}
