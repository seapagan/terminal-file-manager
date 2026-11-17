use super::*;

fn owner_heading(app: &App, width: u16) -> bool {
    use crate::color::Theme;
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
        .contains("Owner")
}

#[test]
fn viewport_work_requires_a_rendered_owner_column_including_pane_borders() {
    let (_root, mut app, _) = loaded(3);
    app.dual = false;
    for (enabled, width, rendered) in [(false, 240, false), (true, 63, false), (true, 64, true)] {
        app.panes[0].columns.owner = enabled;
        let geometry = ui::geometry(&app, Rect::new(0, 0, width, 40));
        assert_eq!(owner_heading(&app, width), rendered);
        let targets = super::super::view::targets(&app, &geometry);
        assert_eq!(targets.len(), if rendered { 3 } else { 0 });
    }
}

#[test]
fn completed_name_restores_owner_column_after_pending_sid_hides_it() {
    use crate::owner::OwnerDisplay;
    let (_root, mut app, _) = loaded(1);
    app.dual = false;
    app.sidebar = false;
    app.popup = false;
    let geometry = ui::geometry(&app, Rect::new(0, 0, 64, 40));
    assert!(geometry.sidebar.is_none());
    assert_eq!(
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names),
        "resolving"
    );
    assert!(owner_heading(&app, 64));
    let gate = Release::new();
    let wait = gate.0.clone();
    let (started, entered) = mpsc::channel();
    let mut runtime = Runtime::using(
        |_, _| Ok("S-1-5-21-123456789-123456789-123456789-1001".into()),
        move |_| {
            started.send(()).unwrap();
            blocked(&wait);
            Name::Resolved("Alice".into())
        },
    );
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names)
            == "S-1-5-21-123456789-123456789-123456789-1001 (resolving)"
    });
    entered.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(!owner_heading(&app, 64));
    drop(gate);
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names) == "Alice"
    });
    assert!(owner_heading(&app, 64));
}

#[test]
fn viewport_owner_columns_respect_independent_pane_settings_and_focus() {
    let (_root, mut app, _) = loaded(3);
    for enabled in [[false, true], [true, false], [false, false], [true, true]] {
        for active in 0..2 {
            app.active = active;
            for (pane, enabled) in app.panes.iter_mut().zip(enabled) {
                pane.columns.owner = enabled;
            }
            let geometry = ui::geometry(&app, Rect::new(0, 0, 240, 40));
            let targets = super::super::view::targets(&app, &geometry);
            for (id, enabled) in enabled.into_iter().enumerate() {
                assert_eq!(
                    targets.iter().filter(|target| target.pane == id).count(),
                    if enabled { 3 } else { 0 }
                );
            }
        }
    }
}

#[test]
fn metadata_alone_resolves_with_disabled_or_responsive_hidden_owner_columns() {
    for (popup, enabled, width) in [
        (true, false, 240),
        (false, false, 240),
        (true, true, 40),
        (false, true, 100),
    ] {
        let (_root, mut app, _) = loaded(3);
        app.dual = false;
        app.popup = popup;
        app.sidebar = !popup;
        for pane in &mut app.panes {
            pane.columns.owner = enabled;
            // A cached wide SID must also fit beside the visible sidebar.
            pane.measurements.owner = 24;
        }
        app.request_metadata(0).unwrap();
        let geometry = ui::geometry(&app, Rect::new(0, 0, width, 40));
        assert!(popup || geometry.sidebar.is_some());
        let targets = super::super::view::targets(&app, &geometry);
        assert_eq!(targets.len(), 1);
        let reads = Arc::new(AtomicUsize::new(0));
        let calls = reads.clone();
        let mut runtime = Runtime::using(
            move |_, _| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok("S-1-5-18".into())
            },
            |_| Name::Resolved("NT AUTHORITY\\SYSTEM".into()),
        );
        until(&mut runtime, &mut app, &geometry, |app| {
            app.details[0]
                .entry
                .as_ref()
                .unwrap()
                .owner_display(crate::owner::OwnerDisplay::Names)
                == "NT AUTHORITY\\SYSTEM"
        });
        assert_eq!(reads.load(Ordering::Relaxed), 1);
    }
}
