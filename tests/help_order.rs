//! Help placement and practical order follow metadata, never enum or key spelling.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{collections::BTreeMap, time::Duration};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller, HelpSection, HelpTarget, help_hits},
    ui,
};

#[test]
fn sections_group_commands_by_the_task_they_perform() {
    use Action::*;
    use HelpSection::*;
    for (action, section) in [
        (Refresh, Navigation),
        (Enter, Navigation),
        (Mark, Selection),
        (PatternSelection, Selection),
        (Copy, Operations),
        (Attributes, Operations),
        (Action::Jobs, HelpSection::Jobs),
        (FocusPane, Panes),
        (Maximise, Panes),
        (Viewer, Views),
        (Editor, Views),
        (Count, Views),
        (ToggleSidebar, Views),
        (Cancel, Application),
        (Diagnostics, Application),
    ] {
        assert_eq!(action.help_section(), section, "{action:?}");
    }
}

#[test]
fn sections_put_common_actions_before_specialised_or_unavailable_commands() {
    use Action::*;
    let entries = Bindings::new(&BTreeMap::new()).unwrap().help_entries();
    let section = |s| {
        entries
            .iter()
            .filter(|e| e.action.help_section() == s)
            .map(|e| e.action)
            .collect::<Vec<_>>()
    };
    assert!(section(HelpSection::Navigation).starts_with(&[Up, Down, Enter, Parent]));
    assert!(section(HelpSection::Navigation).ends_with(&[Path, Zoxide, OppositeZoxide]));
    assert!(section(HelpSection::Selection).starts_with(&[Mark, MarkNext]));
    assert_eq!(
        section(HelpSection::Selection).last(),
        Some(&PatternSelection)
    );
    assert!(
        section(HelpSection::Operations)
            .starts_with(&[Copy, Move, Rename, Mkdir, CreateFile, Trash])
    );
    assert_eq!(section(HelpSection::Operations).last(), Some(&Delete));
    assert_eq!(section(HelpSection::Jobs), vec![Jobs]);
    assert!(section(HelpSection::Panes).starts_with(&[FocusPane, TogglePanes]));
    assert!(section(HelpSection::Views).starts_with(&[Filter, Hidden, Sort, Reverse]));
    assert!(section(HelpSection::Views).ends_with(&[Viewer, Editor]));
    assert!(section(HelpSection::Application).starts_with(&[Help, Cancel, Quit, SaveQuit]));
    assert!(section(HelpSection::Application).ends_with(&[
        Diagnostics,
        DiagnosticsUp,
        DiagnosticsDown
    ]));
}

fn configured() -> Bindings {
    Bindings::new(&BTreeMap::from([
        ("r".into(), Action::Help),
        ("F11".into(), Action::Refresh),
        ("F13".into(), Action::HumanSizes),
        ("F14".into(), Action::ColumnSize),
    ]))
    .unwrap()
}

#[test]
fn configured_keys_change_labels_without_changing_priority_or_search_targets() {
    let entries = configured().help_entries();
    let refresh = entries
        .iter()
        .find(|e| e.action == Action::Refresh)
        .unwrap();
    assert_eq!(refresh.keys, "F11");
    assert_eq!(refresh.description, "Refresh active directory");
    let views: Vec<_> = entries
        .iter()
        .filter(|e| e.action.help_section() == HelpSection::Views)
        .map(|e| e.action)
        .collect();
    assert!(
        views.iter().position(|a| *a == Action::HumanSizes).unwrap()
            < views.iter().position(|a| *a == Action::Viewer).unwrap()
    );
    assert!(
        views.iter().position(|a| *a == Action::ColumnSize).unwrap()
            < views.iter().position(|a| *a == Action::Editor).unwrap()
    );
    for (query, action, section) in [
        ("F11", Action::Refresh, HelpSection::Navigation),
        ("F3", Action::Viewer, HelpSection::Views),
        ("F4", Action::Editor, HelpSection::Views),
    ] {
        let hits = help_hits(&entries, query);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target, HelpTarget::Action(action));
        assert_eq!(hits[0].section, section);
    }
    let visible_ops: Vec<_> = help_hits(&entries, "visible")
        .into_iter()
        .filter(|h| h.section == HelpSection::Operations)
        .map(|h| h.target)
        .collect();
    assert_eq!(
        visible_ops,
        [Action::Copy, Action::Move, Action::Rename].map(HelpTarget::Action)
    );
}

#[test]
fn search_jumps_to_reordered_rows_in_the_original_document_without_dispatch() {
    for width in [20, 40, 80, 120] {
        let mut c = Controller::new(
            App::new("owned-help-order".into()),
            configured(),
            Default::default(),
        );
        c.app.help = true;
        for (query, action) in [
            ("F11", Action::Refresh),
            ("F3", Action::Viewer),
            ("F4", Action::Editor),
        ] {
            jump(&mut c, width, query, action);
        }
        assert!(c.app.operations.outbox.is_empty() && c.app.operations.dialog.is_none());
        assert_eq!(c.app.active, 0);
        assert!(!c.quit);
    }
}

fn jump(c: &mut Controller, width: u16, query: &str, action: Action) {
    let bounds = Rect::new(0, 0, width, 24);
    for event in [
        Event::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        Event::Paste(query.into()),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    ] {
        let g = ui::geometry(&c.app, bounds);
        assert!(c.handle(event, &g, Duration::ZERO).is_empty());
    }
    let target = HelpTarget::Action(action);
    let g = ui::geometry(&c.app, bounds);
    let help = g.help.unwrap();
    let anchor = help.anchors.iter().find(|(t, _)| *t == target).unwrap().1;
    assert_eq!(c.app.help_landing.unwrap().0, target);
    assert_eq!(c.app.overlay_scroll, anchor.min(help.scroll_limit));
    assert!(c.app.help && c.app.help_picker.is_none());
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let body = help.body;
    let text: String = (body.y..body.bottom())
        .flat_map(|y| (body.x..body.right()).map(move |x| &buffer[(x, y)]))
        .flat_map(|cell| cell.symbol().chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    let description = c
        .app
        .help_rows
        .iter()
        .find(|e| e.action == action)
        .unwrap()
        .description;
    let expected: String = description.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(text.contains(&expected), "{width}: missing {description}");
}
