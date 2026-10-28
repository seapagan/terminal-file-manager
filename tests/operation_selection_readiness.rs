use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs,
    input::{Bindings, Controller},
    operations::{Dialog, Intent},
    ui,
};

#[test]
fn inactive_listing_does_not_select_sources_until_the_active_listing_is_ready() {
    let root = tempfile::tempdir().unwrap();
    for name in ["first-owned", "second-owned"] {
        std::fs::write(root.path().join(name), "retained").unwrap();
    }
    let snapshot = fs::list(root.path(), &fs::IgnoreContext::isolated(), &false.into()).unwrap();
    for key in [KeyCode::F(5), KeyCode::F(6)] {
        let mut app = App::new(root.path().into());
        app.dual = true;
        app.complete_listing(1, 1, Ok(snapshot.clone()));
        let mut controller = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        press(&mut controller, KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(controller.app.panes[0].marks.is_empty());
        controller.app.complete_listing(0, 1, Ok(snapshot.clone()));
        assert!(controller.app.panes[0].marks.is_empty());
        // Reproduce the hosted ordering: Ctrl+A preceded the active listing completion.
        press(&mut controller, key, KeyModifiers::NONE);
        assert!(controller.app.operations.dialog.is_none());
        match controller.app.operations.outbox.as_slice() {
            [Intent::Submit(plan, _)] => assert_eq!(plan.operands.len(), 1),
            _ => panic!("Unmarked cursor scope takes the immediate single-source path"),
        }
        controller.app.operations.outbox.clear();
        press(&mut controller, KeyCode::Char('a'), KeyModifiers::CONTROL);
        let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 40));
        assert_eq!(controller.app.active, 0);
        assert_eq!(geometry.selection_counts[0].visible, 2);
        assert_eq!(geometry.selection_counts[1].visible, 0);
        press(&mut controller, key, KeyModifiers::NONE);
        let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
            panic!("Two selected sources must open the batch modal in dual-pane mode");
        };
        assert_eq!(dialog.plan.operands.len(), 2);
        assert!(controller.app.operations.outbox.is_empty());
        assert_batch_modal(&controller);
    }
}

fn assert_batch_modal(controller: &Controller) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &controller.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("2 files selected"), "{text}");
    assert!(text.contains("Enter Run"), "{text}");
}

fn press(controller: &mut Controller, key: KeyCode, modifiers: KeyModifiers) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 40));
    controller.handle(
        Event::Key(KeyEvent::new(key, modifiers)),
        &geometry,
        std::time::Duration::ZERO,
    );
}
