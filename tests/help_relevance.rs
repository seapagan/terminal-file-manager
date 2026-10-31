//! Search should find recognisable commands rather than scattered prose letters.
use terminal_file_manager::input::{Action, Bindings, HelpSection, HelpTarget, help_hits};

fn targets(query: &str) -> Vec<HelpTarget> {
    let entries = Bindings::new(&Default::default()).unwrap().help_entries();
    help_hits(&entries, query)
        .into_iter()
        .map(|hit| hit.target)
        .collect()
}

#[test]
fn rename_search_rejects_unrelated_prose_and_category_matches() {
    assert_eq!(targets("rename"), vec![HelpTarget::Action(Action::Rename)]);
    assert_eq!(targets("rnme"), vec![HelpTarget::Action(Action::Rename)]);
    assert!(targets("zzzzzzzz").is_empty());
}

#[test]
fn compact_fuzzy_names_and_phrase_words_retain_useful_matches() {
    assert_eq!(targets("crtfl")[0], HelpTarget::Action(Action::CreateFile));
    assert_eq!(
        targets("nvgtn")[0],
        HelpTarget::Section(HelpSection::Navigation)
    );
    assert!(targets("split pane").contains(&HelpTarget::Action(Action::Maximise)));
    assert_eq!(targets("split pane"), targets("pane split"));
    assert_eq!(targets("move")[0], HelpTarget::Action(Action::Move));
}

#[test]
fn rankings_and_effective_shortcut_results_remain_deterministic() {
    for query in ["copy", "pane", "back", "rnme", "crtfl"] {
        assert_eq!(targets(query), targets(query));
    }
    let bindings = Bindings::new(&std::collections::BTreeMap::from([(
        "F11".into(),
        Action::Rename,
    )]))
    .unwrap();
    let hits = help_hits(&bindings.help_entries(), "F11");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].target, HelpTarget::Action(Action::Rename));
    assert!(hits[0].keys.contains("F11"));
}
