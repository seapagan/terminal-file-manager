use colored_text::ColorLevel;
use crossterm::event::{
    Event as Input, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller},
    operations::{
        Decision, Event, Job, JobState, Operation, PendingConflict, Plan, Record, Stage, execute,
    },
    ui::{self, OperationTarget},
};

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    std::fs::write(&source, "owned source").unwrap();
    std::fs::create_dir(&target).unwrap();
    let plan = Plan::paths(Operation::Copy, vec![source], Some(target)).unwrap();
    let unknown = unknown_record(&plan, root.path());
    let mut app = App::new(root.path().into());
    for (id, state) in [
        JobState::Completed,
        JobState::Running,
        JobState::Failed,
        JobState::Partial,
        JobState::Unknown,
        JobState::Running,
        JobState::Critical,
    ]
    .into_iter()
    .enumerate()
    {
        app.operations
            .history
            .push(job(id, state, &plan, &unknown, app.now));
    }
    let entry = app.operations.history[0].plan.operands[0].clone();
    let (reply, _) = std::sync::mpsc::sync_channel(1);
    app.operations.conflicts.push_back(PendingConflict {
        id: 5,
        choice: 0,
        all: false,
        reply,
        event: Event::Conflict {
            source: Box::new(entry.clone()),
            destination: Box::new(entry),
            newer_available: false,
        },
    });
    app.operations.show_history = true;
    (root, app)
}

fn job(
    id: usize,
    state: JobState,
    plan: &Plan,
    unknown: &Record,
    now: std::time::SystemTime,
) -> Job {
    Job {
        presentation: None,
        id: id as u64,
        state,
        plan: plan.clone(),
        retry_eligible: matches!(state, JobState::Failed | JobState::Partial),
        record: Some(if state == JobState::Unknown {
            unknown.clone()
        } else {
            Record::default()
        }),
        options: Default::default(),
        progress: None,
        started: now,
        finished: None,
        retry_of: None,
    }
}

fn unknown_record(plan: &Plan, root: &std::path::Path) -> Record {
    let mut payload = None;
    let record = execute(plan, Default::default(), &false.into(), |event| {
        match event {
            Event::OwnedStage { payload: entry, .. } => payload = Some(entry.path),
            Event::Progress {
                stage: Stage::Verified,
                ..
            } => {
                let path = payload.as_ref().unwrap();
                std::fs::rename(path, root.join("held-owned-payload")).unwrap();
                std::fs::write(path, "external replacement").unwrap();
            }
            _ => {}
        }
        Decision::Skip
    });
    assert!(record.reconcilable());
    record
}

fn draw(app: &App, size: (u16, u16), level: ColorLevel) -> (Terminal<TestBackend>, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
    let geometry = ui::geometry(app, Rect::new(0, 0, size.0, size.1));
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    (terminal, geometry)
}

fn line(terminal: &Terminal<TestBackend>, area: Rect) -> String {
    (area.x..area.right())
        .map(|x| terminal.backend().buffer()[(x, area.y)].symbol())
        .collect()
}

#[test]
fn shown_action_rows_are_centred_contiguous_and_match_mouse_regions() {
    let (_root, mut app) = fixture();
    for size in [(44, 18), (80, 24), (120, 40), (300, 100)] {
        for index in 0..app.operations.history.len() {
            app.operations.cursor = index;
            let (terminal, geometry) = draw(&app, size, ColorLevel::NoColor);
            assert_buttons(&terminal, &geometry, size.0, ColorLevel::NoColor);
        }
    }
}

fn assert_buttons(
    terminal: &Terminal<TestBackend>,
    g: &ui::Geometry,
    width: u16,
    level: ColorLevel,
) {
    let buttons: Vec<_> = g
        .operation_controls
        .iter()
        .filter(|b| matches!(b.target, OperationTarget::Key(_)))
        .collect();
    for button in &buttons {
        let text = line(terminal, button.area);
        assert_eq!(
            text,
            format!(
                "[{:<width$}]",
                button.label,
                width = usize::from(button.area.width - 2)
            )
        );
        assert_eq!(
            g.operation_target(button.area.x, button.area.y),
            Some(button.target)
        );
        assert_eq!(
            terminal.backend().buffer()[(button.area.x, button.area.y)].fg,
            Theme::new(level)
                .style(Role::Key)
                .fg
                .unwrap_or(ratatui::style::Color::Reset)
        );
        assert_eq!(
            terminal.backend().buffer()[(button.area.x, button.area.y)].modifier,
            Theme::new(level).style(Role::Key).add_modifier
        );
        assert_row(&buttons, button.area.y, g, width);
    }
}

fn assert_row(buttons: &[&ui::OperationControl], y: u16, g: &ui::Geometry, width: u16) {
    let row: Vec<_> = buttons.iter().filter(|b| b.area.y == y).collect();
    assert!(
        row[0]
            .area
            .x
            .abs_diff(width - row.last().unwrap().area.right())
            <= 1,
        "off-centre buttons at {width}: {:?}",
        row.iter().map(|b| b.area).collect::<Vec<_>>()
    );
    for pair in row.windows(2) {
        assert_eq!(pair[0].area.right() + 1, pair[1].area.x);
    }
    for x in 0..width {
        if !row.iter().any(|b| b.area.contains((x, y).into())) {
            assert_eq!(g.operation_target(x, y), None);
        }
    }
}

#[test]
fn wide_jobs_show_full_context_labels_without_moving_primary_controls() {
    let (_root, mut app) = fixture();
    let (_, initial) = draw(&app, (120, 40), ColorLevel::NoColor);
    for (index, label) in [(1, "C Cancel job"), (2, "R Retry incomplete")] {
        app.operations.cursor = index;
        let (terminal, geometry) = draw(&app, (120, 40), ColorLevel::NoColor);
        let control = geometry
            .operation_controls
            .iter()
            .find(|c| c.label == label)
            .unwrap();
        assert_eq!(line(&terminal, control.area).trim(), format!("[{label}]"));
        for i in 0..2 {
            assert_eq!(
                geometry.operation_controls[i].area,
                initial.operation_controls[i].area
            );
        }
    }
}

#[test]
fn narrow_jobs_show_eligible_shortcuts_inside_the_existing_footer() {
    let (_root, mut app) = fixture();
    for size in [(40, 15), (43, 18), (44, 17), (32, 14)] {
        for (index, labels) in [
            (0, vec![]),
            (1, vec!["C Cancel"]),
            (2, vec!["R Retry"]),
            (3, vec!["R Retry"]),
            (4, vec!["X Reconcile"]),
            (5, vec!["C Cancel", "Enter Conflict"]),
            (6, vec![]),
        ] {
            app.operations.cursor = index;
            let (terminal, g) = draw(&app, size, ColorLevel::NoColor);
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            for label in labels {
                assert!(text.contains(label), "{size:?} job {index}: {text}");
            }
            for (label, available) in [
                ("R Retry", matches!(index, 2 | 3)),
                ("C Cancel", matches!(index, 1 | 5)),
                ("X Reconcile", index == 4),
            ] {
                if !available {
                    assert!(!text.contains(label), "{size:?} job {index}: {text}");
                }
            }
            if index == 4 {
                assert!(text.contains("UNKNOWN"));
            }
            if index == 6 {
                assert!(text.contains("CRITICAL"));
                assert!(!text.contains("R Retry"));
            }
            assert!(
                g.operation_controls
                    .iter()
                    .filter(|b| matches!(b.target, OperationTarget::Key(_)))
                    .all(|b| matches!(
                        b.target,
                        OperationTarget::Key(KeyCode::Char('e') | KeyCode::Esc)
                    ))
            );
        }
    }
}

fn send(c: &mut Controller, g: &ui::Geometry, key: KeyCode) {
    c.handle(
        Input::Key(KeyEvent::new(key, KeyModifiers::NONE)),
        g,
        Duration::ZERO,
    );
}

fn click(c: &mut Controller, g: &ui::Geometry, x: u16, y: u16) {
    c.handle(
        Input::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
        g,
        Duration::ZERO,
    );
}

#[test]
fn contextual_keyboard_and_mouse_commands_keep_their_domain_guards() {
    use terminal_file_manager::operations::Intent;
    let (_root, app) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for index in 0..7 {
        c.app.operations.cursor = index;
        for waiting in [false, true] {
            c.app.operations.waiting_exit = waiting.then_some(false);
            let g = draw(&c.app, (80, 24), ColorLevel::NoColor).1;
            for (key, eligible) in [
                (KeyCode::Char('r'), !waiting && matches!(index, 2 | 3)),
                (KeyCode::Char('c'), matches!(index, 1 | 5)),
                (KeyCode::Char('x'), !waiting && index == 4),
            ] {
                c.app.operations.outbox.clear();
                send(&mut c, &g, key);
                assert_eq!(
                    !c.app.operations.outbox.is_empty(),
                    eligible,
                    "job {index}, {key:?}, waiting={waiting}"
                );
                if let Some(control) = g
                    .operation_controls
                    .iter()
                    .find(|b| b.target == OperationTarget::Key(key))
                {
                    let area = control.area;
                    c.app.operations.outbox.clear();
                    click(&mut c, &g, area.x, area.y);
                    assert_eq!(c.app.operations.outbox.len(), 1);
                }
                for intent in &c.app.operations.outbox {
                    assert!(matches!(
                        intent,
                        Intent::Retry(_) | Intent::Cancel(_) | Intent::Reconcile(_)
                    ));
                }
            }
        }
    }
    assert_enter_opens_conflict(&mut c);
}

fn assert_enter_opens_conflict(c: &mut Controller) {
    c.app.operations.waiting_exit = None;
    c.app.operations.cursor = 5;
    c.app.operations.outbox.clear();
    let g = draw(&c.app, (80, 24), ColorLevel::NoColor).1;
    send(c, &g, KeyCode::Enter);
    assert!(!c.app.operations.show_history);
    assert_eq!(c.app.operations.conflicts.front().unwrap().id, 5);
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn primary_controls_styles_and_empty_band_clicks_stay_consistent() {
    let (_root, app) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let (terminal, g) = draw(&c.app, (80, 24), level);
        assert_buttons(&terminal, &g, 80, level);
        let evidence = g
            .operation_controls
            .iter()
            .find(|b| b.target == OperationTarget::Key(KeyCode::Char('e')))
            .unwrap()
            .area;
        let close = g
            .operation_controls
            .iter()
            .find(|b| b.target == OperationTarget::Key(KeyCode::Esc))
            .unwrap()
            .area;
        for x in evidence.x..close.right() {
            click(&mut c, &g, x, evidence.y + 1);
        }
        assert!(c.app.operations.outbox.is_empty());
        assert!(!c.app.operations.jobs_evidence);
        click(&mut c, &g, evidence.x, evidence.y);
        assert!(c.app.operations.jobs_evidence);
        let (terminal, next) = draw(&c.app, (80, 24), level);
        assert_buttons(&terminal, &next, 80, level);
        let toggle = next
            .operation_controls
            .iter()
            .find(|b| b.target == OperationTarget::Key(KeyCode::Char('e')))
            .unwrap();
        assert_eq!(toggle.area, evidence);
        assert_eq!(toggle.label, "E Summary");
        send(&mut c, &next, KeyCode::Char('E'));
        assert!(!c.app.operations.jobs_evidence);
        send(&mut c, &next, KeyCode::Esc);
        assert!(!c.app.operations.show_history);
        assert!(c.app.operations.outbox.is_empty());
        c.app.operations.show_history = true;
    }
}

#[test]
fn small_unknown_footer_prioritises_restrictions_and_marks_hint_overflow() {
    let (_root, mut app) = fixture();
    app.operations.cursor = 4;
    app.operations.conflicts.front_mut().unwrap().id = 4;
    let (terminal, g) = draw(&app, (26, 12), ColorLevel::NoColor);
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("UNKNOWN: no R/exit"), "{text}");
    assert!(text.contains("X Reconcile · …"), "{text}");
    assert!(!text.contains("R Retry"));
    let e = g
        .operation_controls
        .iter()
        .find(|b| b.target == OperationTarget::Key(KeyCode::Char('e')))
        .unwrap();
    let warning_y = e.area.y - 2;
    let cell = &terminal.backend().buffer()[(e.area.x, warning_y)];
    assert_eq!(
        cell.fg,
        Theme::new(ColorLevel::NoColor)
            .style(Role::Warning)
            .fg
            .unwrap_or(ratatui::style::Color::Reset)
    );
    assert_eq!(
        cell.modifier,
        Theme::new(ColorLevel::NoColor)
            .style(Role::Warning)
            .add_modifier
    );
}

#[test]
fn narrow_hints_are_informational_and_follow_safe_exit_eligibility() {
    let (_root, app) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.app.operations.cursor = 1;
    let (terminal, g) = draw(&c.app, (40, 15), ColorLevel::NoColor);
    let e = g
        .operation_controls
        .iter()
        .find(|b| b.target == OperationTarget::Key(KeyCode::Char('e')))
        .unwrap();
    let y = e.area.y - 2;
    let x = (0..40)
        .find(|&x| terminal.backend().buffer()[(x, y)].symbol() == "C")
        .unwrap();
    assert_eq!(g.operation_target(x, y), None);
    click(&mut c, &g, x, y);
    assert!(c.app.operations.outbox.is_empty());
    send(&mut c, &g, KeyCode::Char('c'));
    assert_eq!(c.app.operations.outbox.len(), 1);
    c.app.operations.outbox.clear();
    c.app.operations.waiting_exit = Some(false);
    for index in [2, 4] {
        c.app.operations.cursor = index;
        let (terminal, _) = draw(&c.app, (40, 15), ColorLevel::NoColor);
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(!text.contains("R Retry"));
        assert!(!text.contains("X Reconcile"));
    }
    c.app.operations.waiting_exit = None;
    c.app.operations.history[4].record = Some(Record::default());
    let (terminal, _) = draw(&c.app, (40, 15), ColorLevel::NoColor);
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(!text.contains("X Reconcile"));
    assert!(text.contains("UNKNOWN"));
}

#[test]
fn one_row_footer_marks_shortcuts_that_cannot_fit() {
    let (_root, mut app) = fixture();
    app.operations.cursor = 5;
    let (terminal, _) = draw(&app, (26, 10), ColorLevel::NoColor);
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("C Cancel · …"), "{text}");
}
