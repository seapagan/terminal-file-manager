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
fn search_uses_effective_shortcuts() {
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

#[test]
fn rank_tiers_and_equal_score_order_follow_the_document() {
    use terminal_file_manager::input::HelpEntry;
    let entries: Vec<_> = [
        (Action::CreateFile, "coppery", ""),
        (Action::Mkdir, "Other", "copy"),
        (Action::Rename, "Safely copy", ""),
        (Action::Trash, "Never copy", ""),
        (Action::Move, "Copy items", ""),
        (Action::Copy, "Copy", ""),
    ]
    .into_iter()
    .map(|(action, description, keys)| HelpEntry {
        action,
        description,
        keys: keys.into(),
    })
    .collect();
    let actual: Vec<_> = help_hits(&entries, "  CoPy  ")
        .into_iter()
        .map(|hit| hit.target)
        .collect();
    assert_eq!(
        actual,
        [
            Action::Copy,
            Action::Move,
            Action::Rename,
            Action::Trash,
            Action::Mkdir,
            Action::CreateFile
        ]
        .map(HelpTarget::Action)
    );
}

#[test]
fn equal_rank_hits_keep_section_order_and_each_sections_entry_order() {
    use terminal_file_manager::input::HelpEntry;
    let entries: Vec<_> = [Action::Jobs, Action::Help, Action::Down, Action::Up]
        .into_iter()
        .map(|action| HelpEntry {
            action,
            description: "Navigation",
            keys: String::new(),
        })
        .collect();
    let targets: Vec<_> = help_hits(&entries, "navigation")
        .into_iter()
        .map(|hit| hit.target)
        .collect();
    assert_eq!(
        targets,
        vec![
            HelpTarget::Section(HelpSection::Navigation),
            HelpTarget::Action(Action::Down),
            HelpTarget::Action(Action::Up),
            HelpTarget::Action(Action::Jobs),
            HelpTarget::Action(Action::Help)
        ]
    );
}
