use super::*;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use terminal_file_manager::{input::Action, notifications::Placement};

fn draw_size(c: &Controller, width: u16, height: u16) -> (ratatui::buffer::Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = ui::geometry(&c.app, terminal.size().unwrap().into());
    terminal
        .draw(|frame| {
            geometry = ui::render(
                frame,
                &c.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            )
        })
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

fn send(c: &mut Controller, code: KeyCode, modifiers: KeyModifiers) {
    let (_, g) = draw(c, 80);
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(code, modifiers)),
            &g,
            Duration::ZERO
        )
        .is_empty()
    );
}

#[test]
fn chronological_cards_stack_outward_from_each_corner_regardless_of_severity() {
    for (placement, left, top) in [
        (Placement::TopLeft, true, true),
        (Placement::TopRight, false, true),
        (Placement::BottomLeft, true, false),
        (Placement::BottomRight, false, false),
    ] {
        let mut c = controller();
        c.app.notifications.placement = placement;
        for (severity, message) in [
            (Severity::Error, "Oldest"),
            (Severity::Warning, "Middle"),
            (Severity::Info, "Newest"),
        ] {
            c.app.notify(severity, message);
        }
        let (buffer, g) = draw_size(&c, 120, 40);
        assert_eq!(
            g.toasts.iter().map(|t| t.event).collect::<Vec<_>>(),
            [2, 1, 0]
        );
        assert_eq!(g.toasts[0].area.x < 30, left);
        for pair in g.toasts.windows(2) {
            assert_eq!(pair[0].area.y < pair[1].area.y, top);
            let (upper, lower) = if top {
                (&pair[0], &pair[1])
            } else {
                (&pair[1], &pair[0])
            };
            assert_eq!(upper.area.bottom() + 1, lower.area.y);
        }
        assert!(text(&buffer, g.toasts[0].area).contains("Newest"));
    }
}

#[test]
fn height_budget_favours_newest_and_overflow_never_returns_after_dismissal_or_resize() {
    let mut c = controller();
    for i in 0..40 {
        c.app.notify(Severity::Info, format!("Event {i}"));
    }
    let (_, tall) = draw_size(&c, 120, 200);
    assert!(tall.toasts.len() > 16);
    let (_, small) = draw_size(&c, 120, 40);
    assert!(small.toasts.len() > 3);
    let occupied =
        small.toasts.iter().map(|t| t.area.height).sum::<u16>() + small.toasts.len() as u16 - 1;
    assert!(occupied <= 32);
    assert_eq!(small.toasts[0].event, 39);
    assert_eq!(draw_size(&c, 120, 200).1.toasts.len(), small.toasts.len());
    c.app.notifications.dismiss_toasts();
    assert!(draw_size(&c, 120, 200).1.toasts.is_empty());
    assert_eq!(c.app.notifications.history.len(), 40);
}

#[test]
fn history_never_covers_either_critical_status_row_on_narrow_terminals() {
    let (_root, mut c) = incident();
    c.app.notifications.open = true;
    c.app.notifications.details = true;
    c.app.notifications.history[0].text = "detail ".repeat(200);
    let (buffer, g) = draw_size(&c, 20, 18);
    assert_eq!(g.status.height, 2);
    assert!(
        !g.notification_history
            .unwrap()
            .shell
            .outer
            .intersects(g.status)
    );
    assert!(text(&buffer, g.status).contains("CRITICAL"));
}

fn incident() -> (tempfile::TempDir, Controller) {
    use terminal_file_manager::{
        fs::EntryKind,
        operations::{EntryResult, Outcome, Plan, Record, Update},
    };
    let (root, mut c) = loaded();
    c.register_operation(
        7,
        Plan::mkdir(root.path().join("owned")),
        Default::default(),
        None,
    );
    c.complete_operation(Update::Finished {
        id: 7,
        record: Record {
            entries: vec![EntryResult {
                source: root.path().join("owned"),
                destination: None,
                kind: EntryKind::Directory,
                outcome: Outcome::SafetyIncident,
                bytes: 0,
                checksum_verified: false,
                created_directory: false,
                detail: "Retained safety incident".into(),
                trash: None,
                recovery: None,
            }],
        },
    });
    (root, c)
}

#[test]
fn dismiss_shortcuts_use_visible_chronology_and_clear_grouped_job_toasts() {
    let mut c = controller();
    c.app.notify(Severity::Error, "Oldest");
    for id in [7, 8] {
        c.app
            .notifications
            .record("Grouped".into(), c.app.now, Severity::Info, Some(id));
    }
    send(&mut c, KeyCode::Char('k'), KeyModifiers::ALT);
    assert_eq!(c.app.notifications.visible(), [2]);
    send(&mut c, KeyCode::Char('k'), KeyModifiers::ALT);
    assert!(c.app.notifications.visible().is_empty());
    assert_eq!(c.app.notifications.history.len(), 3);
    c.app
        .notifications
        .record("Grouped".into(), c.app.now, Severity::Info, Some(9));
    assert_eq!(draw(&c, 80).1.toasts.len(), 1);
    send(
        &mut c,
        KeyCode::Char('k'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert!(c.app.notifications.visible().is_empty());
    assert_eq!(c.app.notifications.history.len(), 4);
}

#[test]
fn global_dismissal_preserves_filter_help_pattern_jobs_and_operation_edits() {
    for action in [
        Action::Filter,
        Action::Help,
        Action::PatternSelection,
        Action::Jobs,
        Action::Mkdir,
        Action::Metadata,
        Action::ViewOptions,
    ] {
        let (_root, mut c) = loaded();
        let (_, g) = draw(&c, 80);
        c.dispatch(action, &g);
        send(&mut c, KeyCode::Char('x'), KeyModifiers::NONE);
        let before = surface_state(&c);
        c.app.notify(Severity::Error, "Foreground");
        send(
            &mut c,
            KeyCode::Char('k'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert!(c.app.notifications.visible().is_empty(), "{action:?}");
        assert_eq!(before, surface_state(&c));
        assert!(!c.quit);
        assert!(c.app.operations.outbox.is_empty());
    }
}

#[test]
fn toast_click_dismisses_foreground_without_opening_history_or_submitting_dialog() {
    let (root, mut c) = loaded();
    let collision = root.path().join("exists");
    std::fs::write(&collision, "preserved").unwrap();
    for _ in 0..2 {
        complete(
            &mut c,
            terminal_file_manager::operations::Plan::create_file(collision.clone()),
        );
    }
    let (_, g) = draw(&c, 80);
    c.dispatch(Action::Mkdir, &g);
    send(&mut c, KeyCode::Char('p'), KeyModifiers::NONE);
    let (buffer, g) = draw(&c, 80);
    assert_eq!(g.toasts.len(), 1);
    let toast = &g.toasts[0];
    assert!(text(&buffer, toast.area).contains("×2"));
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: toast.area.x + 2,
            row: toast.area.y + 1,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Duration::ZERO,
    );
    assert!(!c.app.notifications.open);
    assert!(draw(&c, 80).1.toasts.is_empty());
    let Some(terminal_file_manager::operations::Dialog::Operation(dialog)) =
        &c.app.operations.dialog
    else {
        panic!("dialog lost")
    };
    assert_eq!(dialog.text, "p");
    assert!(c.app.operations.outbox.is_empty());
    assert_eq!(c.app.notifications.history.len(), 2);
    assert_eq!(c.app.operations.history.len(), 2);
    assert!(
        c.app
            .operations
            .history
            .iter()
            .all(|job| job.record.is_some())
    );
}

#[test]
fn history_live_follows_newest_but_preserves_older_details_and_consumes_toasts() {
    let mut c = controller();
    c.app.notify(Severity::Info, "First");
    let (_, g) = draw(&c, 80);
    c.dispatch(Action::Notifications, &g);
    c.app.notify(Severity::Warning, "Incoming");
    assert_eq!(c.app.notifications.cursor, 1);
    let (buffer, g) = draw(&c, 80);
    assert!(g.toasts.is_empty());
    assert!(text(&buffer, g.notification_history.unwrap().shell.body).contains("> Warning"));
    c.app.notifications.cursor = 0;
    c.app.notifications.details = true;
    c.app.notifications.scroll = 2;
    c.app.notify(Severity::Error, "Another");
    c.app
        .notifications
        .notify("First".into(), c.app.now, Severity::Info);
    assert_eq!(c.app.notifications.cursor, 0);
    assert_eq!(c.app.notifications.scroll, 2);
    c.app
        .notify(Severity::Info, "Still inspecting older details");
    assert_eq!(c.app.notifications.cursor, 0);
    assert_eq!(c.app.notifications.scroll, 2);
    c.app.notifications.scroll = 0;
    c.app.now += Duration::from_secs(60);
    let (buffer, g) = draw(&c, 80);
    let detail = text(&buffer, g.notification_history.unwrap().shell.body);
    assert!(detail.contains("2 occurrences"), "{detail}");
    assert!(detail.contains("minute"), "{detail}");
    send(&mut c, KeyCode::Esc, KeyModifiers::NONE);
    assert!(draw(&c, 80).1.toasts.is_empty());
    assert_eq!(c.app.notifications.history.len(), 4);
}

#[test]
fn accepted_listing_errors_keep_error_severity() {
    let mut c = controller();
    c.app
        .complete_listing(0, 1, Err("Unreadable directory".into()));
    assert_eq!(c.app.notifications.history[0].severity, Severity::Error);
}

#[test]
fn repeating_an_older_event_does_not_interrupt_following_new_history_entries() {
    let mut c = controller();
    c.app.notify(Severity::Info, "Older");
    c.app.notify(Severity::Info, "Newest entry");
    let (_, g) = draw(&c, 80);
    c.dispatch(Action::Notifications, &g);
    assert_eq!(c.app.notifications.cursor, 1);
    c.app.notify(Severity::Info, "Older");
    assert_eq!(c.app.notifications.cursor, 1);
    c.app.notify(Severity::Info, "New arrival");
    assert_eq!(c.app.notifications.cursor, 2);
    assert!(draw(&c, 80).1.toasts.is_empty());
}

#[test]
fn remapped_dismiss_actions_work_inside_an_editor_without_inserting_characters() {
    let settings = terminal_file_manager::settings::Settings::parse("[bindings]\n'Alt+k' = 'up'\n'Ctrl+Alt+k' = 'down'\n'F11' = 'dismiss-toast'\n'F12' = 'dismiss-toasts'").unwrap();
    let mut c = Controller::new(
        App::new("owned".into()),
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    );
    let (_, g) = draw(&c, 80);
    c.dispatch(Action::Filter, &g);
    for message in ["First", "Second"] {
        c.app.notify(Severity::Info, message);
    }
    send(&mut c, KeyCode::F(11), KeyModifiers::NONE);
    assert_eq!(c.app.notifications.visible(), [1]);
    send(&mut c, KeyCode::F(12), KeyModifiers::NONE);
    assert!(c.app.notifications.visible().is_empty());
    assert!(c.app.panes[0].filter.is_empty());
}

#[test]
fn foreground_cards_keep_the_same_rectangles_above_underlying_surfaces() {
    for action in [
        Action::Help,
        Action::Jobs,
        Action::Metadata,
        Action::PatternSelection,
        Action::Mkdir,
    ] {
        let (_root, mut c) = loaded();
        for i in 0..3 {
            c.app
                .notifications
                .notify(format!("Event {i}"), c.app.now, Severity::Info);
        }
        let (_, before) = draw(&c, 80);
        c.dispatch(action, &before);
        let (buffer, after) = draw(&c, 80);
        assert_eq!(
            before.toasts.iter().map(|t| t.area).collect::<Vec<_>>(),
            after.toasts.iter().map(|t| t.area).collect::<Vec<_>>(),
            "{action:?}"
        );
        for toast in &after.toasts {
            assert_eq!(buffer[(toast.area.x, toast.area.y)].symbol(), "▌");
        }
    }
    let (_root, mut c) = incident();
    let (buffer, g) = draw(&c, 20);
    assert!(g.toasts[0].area.intersects(g.status));
    assert_eq!(buffer[(g.toasts[0].area.x, g.status.y)].symbol(), "▌");
    send(
        &mut c,
        KeyCode::Char('k'),
        KeyModifiers::ALT | KeyModifiers::CONTROL,
    );
    assert!(c.app.operations.incidents());
    assert!(c.app.operations.history[0].record.is_some());
    assert!(!c.quit);
}

#[test]
fn genuine_metadata_worker_failure_notifies_but_stale_failures_do_not() {
    let (root, mut c) = loaded();
    let path = root.path().join("owned");
    std::fs::write(&path, "data").unwrap();
    let request = c.app.request_listing(0, root.path().into());
    c.app.complete_listing(
        0,
        request.generation,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &false.into(),
        )
        .unwrap()),
    );
    let request = c.app.request_metadata(0).unwrap();
    assert!(c.app.complete_failure(&request, "Worker failed".into()));
    assert_eq!(c.app.notifications.history[0].severity, Severity::Error);
    let mut stale = request;
    stale.generation += 1;
    assert!(
        !c.app
            .complete_failure(&stale, "Stale worker failure".into())
    );
    assert_eq!(c.app.notifications.history.len(), 1);
}

#[test]
fn toast_over_a_run_button_consumes_click_without_submitting_the_operation() {
    let (_root, mut c) = loaded();
    let (_, g) = draw(&c, 80);
    c.dispatch(Action::Mkdir, &g);
    send(&mut c, KeyCode::Char('p'), KeyModifiers::NONE);
    for i in 0..3 {
        c.app.notify(Severity::Info, format!("Cover {i}"));
    }
    let (_, g) = draw(&c, 80);
    let button = g
        .operation_controls
        .iter()
        .find(|control| {
            control.target == ui::OperationTarget::Key(KeyCode::Enter) && control.enabled
        })
        .unwrap();
    let toast = g
        .toasts
        .iter()
        .find(|toast| toast.area.intersects(button.area))
        .unwrap();
    let hit = toast.area.intersection(button.area);
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Duration::ZERO,
    );
    assert!(c.app.operations.dialog.is_some());
    assert!(c.app.operations.outbox.is_empty());
    assert!(!c.app.notifications.open);
    assert_eq!(c.app.notifications.visible().len(), 2);
}

#[test]
fn passive_metadata_diagnostics_remain_inline_without_toasts_or_history() {
    let root = tempfile::tempdir().unwrap();
    for name in ["first", "second"] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    let mut c = controller();
    c.app = App::new(root.path().into());
    c.app.complete_listing(
        0,
        1,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &false.into(),
        )
        .unwrap()),
    );
    for name in ["first", "second"] {
        c.app.panes[0].cursor = Some(root.path().join(name));
        let request = c.app.request_metadata(0).unwrap();
        let mut entry = c.app.panes[0].current().unwrap().clone();
        entry.error = Some("Broken link diagnostic".into());
        assert!(c.app.complete_metadata(&request, entry, None));
        c.app.popup = true;
        let (buffer, _) = draw(&c, 100);
        assert!(text(&buffer, buffer.area).contains("Broken link diagnostic"));
        assert!(c.app.notifications.history.is_empty());
        assert!(c.app.notifications.visible().is_empty());
    }
}

fn surface_state(c: &Controller) -> String {
    let edit = match &c.app.operations.dialog {
        Some(terminal_file_manager::operations::Dialog::Operation(d)) => d.text.clone(),
        _ => String::new(),
    };
    format!(
        "{}|{:?}|{}|{}|{}|{}|{}",
        edit,
        c.app.view_menu,
        c.app.pattern.is_some(),
        c.app.help,
        c.app.popup,
        c.app.operations.show_history,
        c.app.panes[0].filter
    )
}

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller();
    c.app = App::new(root.path().into());
    c.app.complete_listing(
        0,
        1,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &false.into(),
        )
        .unwrap()),
    );
    (root, c)
}
