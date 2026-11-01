use super::{HelpEntry, HelpSection};
use crate::input::Action;
use std::{cell::RefCell, fmt::Write as _, sync::Arc};

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
    results: RefCell<Option<CachedHits>>,
}

#[derive(Clone, Debug)]
struct CachedHits {
    query: String,
    entries: Vec<HelpEntry>,
    hits: Arc<[HelpHit]>,
}

impl HelpPicker {
    /// Share one ranked result set until the query or effective entries change.
    pub fn hits(&self, entries: &[HelpEntry]) -> Arc<[HelpHit]> {
        let mut cache = self.results.borrow_mut();
        if let Some(cached) = cache.as_mut()
            && cached.entries == entries
        {
            if cached.query != self.query {
                cached.query.clone_from(&self.query);
                cached.hits = help_hits(entries, &self.query).into();
            }
            return Arc::clone(&cached.hits);
        }
        let hits: Arc<[HelpHit]> = help_hits(entries, &self.query).into();
        *cache = Some(CachedHits {
            query: self.query.clone(),
            entries: entries.to_vec(),
            hits: Arc::clone(&hits),
        });
        hits
    }
}

/// Exact/title matches precede shortcut and fuzzy description matches; ties retain document order.
pub fn help_hits(entries: &[HelpEntry], query: &str) -> Vec<HelpHit> {
    let query = query.trim().to_lowercase();
    let action_query = query.replace(' ', "");
    let mut hits = Vec::new();
    for section in HelpSection::ALL {
        let title = section.title();
        if let Some(rank) = rank(&query, &action_query, title, "", "") {
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
    }
    if !query.is_empty() {
        let mut action = String::new();
        for entry in entries {
            action.clear();
            write!(&mut action, "{:?}", entry.action).expect("formatting into String");
            action.make_ascii_lowercase();
            if let Some(score) = rank(
                &query,
                &action_query,
                entry.description,
                &entry.keys,
                &action,
            ) {
                hits.push((
                    score,
                    HelpHit {
                        target: HelpTarget::Action(entry.action),
                        title: entry.description.into(),
                        keys: entry.keys.clone(),
                        section: entry.action.help_section(),
                    },
                ));
            }
        }
    }
    hits.sort_by_key(|(score, hit)| (*score, hit.section));
    hits.into_iter().map(|(_, hit)| hit).collect()
}

// Fields never share a fuzzy span; category text cannot match unrelated commands.
fn rank(
    query: &str,
    action_query: &str,
    title: &str,
    keys: &str,
    action: &str,
) -> Option<(u8, usize)> {
    let title = text_rank(query, &title.to_lowercase());
    let action = text_rank(action_query, action);
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

#[cfg(test)]
mod tests {
    use super::fuzzy_gap;

    fn rank(query: &str, title: &str, keys: &str, action: &str) -> Option<(u8, usize)> {
        super::rank(query, &query.replace(' ', ""), title, keys, action)
    }

    #[test]
    fn rank_prefers_titles_then_shortcuts_then_word_abbreviations() {
        assert_eq!(rank("copy", "Copy", "", ""), Some((0, 0)));
        assert_eq!(rank("copy", "Copy files", "", ""), Some((1, 0)));
        assert_eq!(rank("copy", "Safely copy", "", ""), Some((2, 0)));
        assert_eq!(rank("f11", "Rename", "F11", "rename"), Some((3, 0)));
        assert_eq!(rank("rnme", "Rename", "", "rename"), Some((4, 2)));
        assert_eq!(rank("ry", "Rename safely", "", ""), None);
        assert_eq!(rank("copy", "Co", "py", ""), None);
    }

    #[test]
    fn fuzzy_matches_require_a_word_start_and_sufficient_letter_density() {
        assert_eq!(fuzzy_gap("rename", "rnme"), Some(2));
        assert_eq!(fuzzy_gap("rename", "nme"), None);
        assert_eq!(fuzzy_gap("rabcdefghimn", "rmn"), None);
        assert_eq!(fuzzy_gap("rename", "rmz"), None);
        assert_eq!(fuzzy_gap("éclair", "écr"), Some(3));
        assert_eq!(fuzzy_gap("", "rnme"), None);
    }
}
