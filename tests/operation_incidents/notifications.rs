use super::*;

#[test]
fn expired_toasts_and_history_browsing_preserve_critical_and_unknown_exit_guards() {
    for (outcome, label) in [
        (Outcome::SafetyIncident, "CRITICAL"),
        (Outcome::OutcomeUnknown, "UNKNOWN"),
    ] {
        let (_root, mut c) = fixture();
        finish(&mut c, 1, outcome);
        c.app.notifications.advance(Duration::from_secs(60));
        c.app.status = "Routine context".into();
        assert_persistent(&c.app, label);
        c.app.notifications.open = true;
        c.app.context_help = Some(terminal_file_manager::input::ContextHelp {
            topic: terminal_file_manager::input::HelpTopic::Jobs,
            scroll: 0,
        });
        event(
            &mut c,
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        );
        assert!(!c.quit);
        assert!(c.app.context_help.is_none());
        assert!(c.app.operations.unresolved());
        assert_eq!(
            c.app.operations.history[0].record.as_ref().unwrap().entries[0].outcome,
            outcome
        );
    }
}

fn assert_persistent(app: &App, label: &str) {
    use colored_text::ColorLevel;
    use terminal_file_manager::color::Theme;
    for width in [20, 40, 80, 120] {
        let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 24)).unwrap();
        let g = ui::geometry(app, Rect::new(0, 0, width, 24));
        t.draw(|f| {
            ui::render(f, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains(label), "{text}");
        assert!((1..=2).contains(&g.status.height));
        if g.status.height == 2 {
            let second: String = (0..width)
                .map(|x| t.backend().buffer()[(x, g.status.y + 1)].symbol())
                .collect();
            assert!(second.contains("F9 details"), "{second}");
        }
        assert!(
            g.status_controls
                .iter()
                .all(|(area, _)| area.right() <= width)
        );
    }
}
