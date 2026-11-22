use super::*;
use terminal_file_manager::input::HelpTopic;

fn workflow_state(c: &Controller) -> (usize, &'static str, bool) {
    let flow = c.app.footer_workflow.as_ref().unwrap();
    (
        flow.stage_index(),
        flow.highlighted().unwrap().id,
        flow.can_back(),
    )
}

fn assert_help_rendered(c: &Controller) {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 24)).unwrap();
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
    assert!(text.contains("Copy Path Help"));
}

#[test]
fn both_help_keys_preserve_copy_stages_and_escape_cancels_one_layer() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "owned").unwrap();
    for (keys, help) in [
        ("y", KeyCode::Char('?')),
        ("yh", KeyCode::Char('?')),
        ("y", KeyCode::F(1)),
        ("yh", KeyCode::F(1)),
    ] {
        let mut c = controller(root.path());
        c.app.panes[0].cursor = Some(root.path().join("file"));
        sequence(&mut c, keys);
        let before = workflow_state(&c);
        let base = c.app.relative_path_base.clone();
        send(&mut c, help);
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::PathCopy
        );
        assert!(!c.reconcile_footer());
        assert_help_rendered(&c);
        for code in [KeyCode::Enter, KeyCode::Char('s'), KeyCode::F(5)] {
            send(&mut c, code);
        }
        send(&mut c, KeyCode::End);
        assert!(c.app.context_help.as_ref().unwrap().scroll > 0);
        assert_eq!(workflow_state(&c), before);
        assert!(c.path_copy_request.is_none() && c.path_base_request.is_none());
        assert_eq!(c.app.relative_path_base, base);
        send(&mut c, KeyCode::Esc);
        assert!(c.app.context_help.is_none());
        assert_eq!(workflow_state(&c), before);
        send(&mut c, KeyCode::Esc);
        assert!(c.app.footer_workflow.is_none());
        assert!(c.path_copy_request.is_none() && c.path_base_request.is_none());
    }
}

#[test]
fn help_round_trip_does_not_consume_workflow_history_or_change_relative_copy() {
    let root = tempfile::tempdir().unwrap();
    for help in [KeyCode::Char('?'), KeyCode::F(1)] {
        let mut c = controller(root.path());
        sequence(&mut c, "yc");
        for _ in 2..terminal_file_manager::workflow::MAX_INPUTS - 1 {
            send(&mut c, KeyCode::Up);
        }
        send(&mut c, help);
        send(&mut c, KeyCode::Esc);
        send(&mut c, KeyCode::Char('r'));
        assert_eq!(c.path_copy_request.take().unwrap().prepare().unwrap(), ".");
        assert_eq!(c.app.relative_path_base, root.path());
    }
}

#[test]
fn detailed_help_opens_with_f1_and_question_only_in_the_active_workflow() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    send(&mut c, KeyCode::Char('?'));
    assert!(c.app.help && c.app.context_help.is_none());
    send(&mut c, KeyCode::Esc);
    sequence(&mut c, "y");
    send(&mut c, KeyCode::F(1));
    assert!(!c.app.help);
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::PathCopy
    );
    send(&mut c, KeyCode::Esc);
    send(&mut c, KeyCode::Char('?'));
    let text = HelpTopic::PathCopy.text(&c.app);
    let prose: String = text
        .iter()
        .flatten()
        .map(|fragment| fragment.text())
        .collect();
    for expected in [
        "wl-copy",
        "xclip/xsel",
        "pbcopy",
        "Windows clipboard",
        "OSC 52",
        "74,991",
        "16 MiB",
    ] {
        assert!(prose.contains(expected), "{expected}");
    }
}

fn mouse(c: &mut Controller, x: u16, y: u16) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    assert!(
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            Default::default()
        )
        .is_empty()
    );
}

#[test]
fn title_close_hits_only_its_cells_and_preserves_both_copy_stages() {
    let root = tempfile::tempdir().unwrap();
    for keys in ["y", "yc"] {
        for offset in 0..3 {
            let mut c = controller(root.path());
            sequence(&mut c, keys);
            let before = workflow_state(&c);
            let base = c.app.relative_path_base.clone();
            send(&mut c, KeyCode::F(1));
            let shell = ui::geometry(&c.app, Rect::new(0, 0, 120, 24))
                .context_help
                .unwrap()
                .shell;
            let close = shell.close.unwrap();
            for (x, y) in [
                (close.x - 1, close.y),
                (close.right(), close.y),
                (close.x, close.y + 1),
            ] {
                mouse(&mut c, x, y);
                assert!(c.app.context_help.is_some());
            }
            mouse(&mut c, close.x + offset, close.y);
            assert!(c.app.context_help.is_none());
            assert_eq!(workflow_state(&c), before);
            assert_eq!(c.app.relative_path_base, base);
            assert!(c.path_copy_request.is_none() && c.path_base_request.is_none());
            send(&mut c, KeyCode::Esc);
            assert!(c.app.footer_workflow.is_none());
        }
    }
}

#[test]
fn mouse_close_does_not_consume_copy_input_history() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "yc");
    for _ in 2..terminal_file_manager::workflow::MAX_INPUTS - 1 {
        send(&mut c, KeyCode::Up);
    }
    send(&mut c, KeyCode::F(1));
    let close = ui::geometry(&c.app, Rect::new(0, 0, 120, 24))
        .context_help
        .unwrap()
        .shell
        .close
        .unwrap();
    mouse(&mut c, close.x + 1, close.y);
    send(&mut c, KeyCode::Char('r'));
    assert_eq!(c.path_copy_request.take().unwrap().prepare().unwrap(), ".");
}
