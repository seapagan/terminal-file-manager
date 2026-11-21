use super::*;

#[test]
fn delete_aliases_preserve_existing_operation_planning() {
    use terminal_file_manager::{input::Action, operations::Operation};
    for (alias, fallback, action, operation) in [
        ("Delete", "F8", Action::Trash, Operation::Trash),
        (
            "Shift+Delete",
            "Shift+F8",
            Action::Delete,
            Operation::Delete,
        ),
    ] {
        let (_root, mut c) = controller();
        let bindings = Bindings::new(&Default::default()).unwrap();
        for chord in [alias, fallback] {
            let (code, modifiers) = terminal_file_manager::input::chord(chord).unwrap();
            assert_eq!(
                bindings.action(KeyEvent::new(code, modifiers)),
                Some(action)
            );
        }
        key(&mut c, alias);
        let terminal_file_manager::operations::Dialog::Operation(dialog) =
            c.app.operations.dialog.as_ref().unwrap()
        else {
            panic!("operation dialog")
        };
        assert_eq!(dialog.plan.operation, operation);
        assert!(!c.app.operations.outbox.iter().any(|intent| matches!(
            intent,
            terminal_file_manager::operations::Intent::Submit { .. }
        )));
    }
}

#[test]
fn empty_filtered_pane_explains_no_matches_without_extra_status_rows() {
    let (_root, mut c) = controller();
    c.app.panes[0].set_filter("missing".into());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 16)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &c.app,
                &terminal_file_manager::color::Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("FILTER: missing"));
    assert!(text.contains("No matches"));
    assert!(!text.contains("No visible entries"));
}

#[test]
fn alternate_metadata_binding_opens_popup_despite_sidebar_preference() {
    let (_root, mut c) = controller();
    c.preferences.metadata_presentation =
        terminal_file_manager::settings::MetadataPresentation::Sidebar;
    key(&mut c, "Alt+o");
    assert!(c.app.popup);
    assert!(!c.app.sidebar);
    key(&mut c, "Alt+o");
    assert!(!c.app.popup);
    key(&mut c, "Alt+Enter");
    assert!(c.app.popup);
    let bindings = Bindings::new(&std::collections::BTreeMap::from([(
        "Alt+u".into(),
        terminal_file_manager::input::Action::Metadata,
    )]))
    .unwrap();
    let (code, modifiers) = terminal_file_manager::input::chord("Alt+u").unwrap();
    assert_eq!(
        bindings.action(KeyEvent::new(code, modifiers)),
        Some(terminal_file_manager::input::Action::Metadata)
    );
}

#[test]
fn preferred_metadata_presentation_keeps_both_direct_shortcuts() {
    let (_root, mut controller) = controller();
    controller.preferences =
        terminal_file_manager::settings::Settings::parse("metadata_presentation = 'sidebar'")
            .unwrap();
    key(&mut controller, "F2");
    assert!(controller.app.sidebar);
    assert!(!controller.app.popup);
    key(&mut controller, "Alt+Enter");
    assert!(controller.app.popup);
    key(&mut controller, "Esc");
    key(&mut controller, "Alt+i");
    assert!(!controller.app.sidebar);
}
