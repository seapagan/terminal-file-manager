use super::*;
use crate::{app::PatternSelection, input::Bindings, ui::PatternControl};

fn controller() -> Controller {
    Controller::new(
        App::new("owned-topic-routing".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

#[test]
fn current_topic_follows_the_visible_layer_instead_of_a_background_dialog() {
    let mut c = controller();
    assert_eq!(HelpTopic::current(&c.app), None);
    c.app.pattern = Some(PatternSelection::new(&c.app.panes[0], 0));
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::Pattern));
    c.app.operations.waiting_exit = Some(false);
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::Exit));
    c.app.operations.show_history = true;
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::Jobs));
    c.app.operations.dialog = Some(Dialog::IncidentExit {
        save: false,
        typed: String::new(),
    });
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::Incident));
    c.app.profile_menu = Some(0);
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::ViewProfiles));
    c.app.profile_name = Some(String::new());
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::SaveProfile));
    c.app.view_menu = Some(0);
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::ViewOptions));
    c.app.popup = true;
    assert_eq!(HelpTopic::current(&c.app), Some(HelpTopic::Metadata));
    c.app.help = true;
    assert_eq!(HelpTopic::current(&c.app), None);
    c.app.help = false;
    c.app.diagnostics = true;
    assert_eq!(HelpTopic::current(&c.app), None);
}

#[test]
fn editable_topics_reserve_question_but_always_offer_f1() {
    let mut c = controller();
    let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let f1 = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    for (operation, editable) in [
        (Operation::Copy, true),
        (Operation::Move, true),
        (Operation::Rename, true),
        (Operation::Mkdir, true),
        (Operation::CreateFile, true),
        (Operation::Delete, false),
        (Operation::Trash, false),
    ] {
        let topic = HelpTopic::Operation(operation);
        assert_eq!(editing(&c.app, topic), editable);
        assert_eq!(opens_with_key(&c, question, topic), !editable);
        assert!(opens_with_key(&c, f1, topic));
    }
    for topic in [HelpTopic::Incident, HelpTopic::SaveProfile] {
        assert!(editing(&c.app, topic));
        assert!(!opens_with_key(&c, question, topic));
        assert!(opens_with_key(&c, f1, topic));
    }
    c.app.pattern = Some(PatternSelection::new(&c.app.panes[0], 0));
    assert!(!opens_with_key(&c, question, HelpTopic::Pattern));
    c.app.pattern.as_mut().unwrap().focus = PatternControl::Cancel;
    assert!(opens_with_key(&c, question, HelpTopic::Pattern));
}

#[test]
fn help_requires_an_unmodified_press_and_respects_question_rebinding() {
    let mut c = controller();
    for code in [KeyCode::F(1), KeyCode::Char('?')] {
        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            assert!(!opens_with_key(
                &c,
                KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind),
                HelpTopic::Jobs
            ));
        }
        for modifier in [KeyModifiers::ALT, KeyModifiers::CONTROL] {
            assert!(!opens_with_key(
                &c,
                KeyEvent::new(code, modifier),
                HelpTopic::Jobs
            ));
        }
    }
    assert!(opens_with_key(
        &c,
        KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
        HelpTopic::Jobs
    ));
    c.bindings = Bindings::new(&[("?".into(), crate::input::Action::Jobs)].into()).unwrap();
    assert!(!opens_with_key(
        &c,
        KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
        HelpTopic::Jobs
    ));
    assert!(opens_with_key(
        &c,
        KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
        HelpTopic::Jobs
    ));
}

#[test]
fn context_resize_clamps_parent_scroll_and_ends_divider_drag() {
    let mut c = controller();
    c.app.popup = true;
    c.app.context_help = Some(ContextHelp {
        topic: HelpTopic::Metadata,
        scroll: u16::MAX,
    });
    c.app.overlay_scroll = u16::MAX;
    c.app.metadata_scroll = u16::MAX;
    c.dragging_divider = true;
    let area = ratatui::layout::Rect::new(0, 0, 120, 40);
    let g = crate::ui::geometry(&c.app, area);
    assert!(handle(&mut c, &Event::Resize(120, 40), &g));
    assert!(!c.dragging_divider);
    assert_eq!(c.app.overlay_scroll, g.overlay_scroll_limit);
    assert_eq!(c.app.metadata_scroll, g.metadata_scroll_limit);
    assert_eq!(
        c.app.context_help.as_ref().unwrap().scroll,
        g.context_help.unwrap().scroll_limit
    );
}
