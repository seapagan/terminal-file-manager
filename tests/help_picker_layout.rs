//! Picker rows and query use one measured, non-overlapping geometry.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller, HelpTarget, chord},
    ui,
};

fn controller(query: &str) -> Controller {
    let mut c = Controller::new(
        App::new("owned-picker-layout".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.app.help = true;
    c.app.help_picker = Some(Default::default());
    c.app.help_picker.as_mut().unwrap().query = query.into();
    c
}
fn draw(c: &Controller, bounds: Rect, level: ColorLevel) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, &c.app, &Theme::new(level));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}
fn row(buffer: &Buffer, area: Rect) -> String {
    (area.x..area.right())
        .map(|x| buffer[(x, area.y)].symbol())
        .collect()
}
fn mouse(c: &mut Controller, bounds: Rect, area: Rect) {
    let g = ui::geometry(&c.app, bounds);
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Duration::ZERO,
    );
}
#[test]
fn empty_query_shows_single_line_sections_below_a_separate_input() {
    let c = controller("");
    let bounds = Rect::new(0, 0, 80, 24);
    let p = ui::geometry(&c.app, bounds).help.unwrap().picker.unwrap();
    assert_eq!(p.controls.len(), 7);
    assert!(p.shell.body.y > p.query.bottom());
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        let b = draw(&c, bounds, level);
        assert!(row(&b, p.query).starts_with("Search: "));
        assert!(row(&b, p.separator).chars().all(|ch| ch == '─'));
        let cursor = Theme::new(level).style(Role::Cursor);
        let query = &b[(p.query.x, p.query.y)];
        assert_ne!(
            (query.bg, query.modifier),
            (cursor.bg.unwrap_or_default(), cursor.add_modifier)
        );
        for (area, index) in &p.controls {
            assert_eq!(area.height, 1);
            assert!(matches!(p.hits[*index].target, HelpTarget::Section(_)));
            assert!(row(&b, *area).contains(&p.hits[*index].title));
        }
    }
}
#[test]
fn query_and_separator_clicks_do_not_jump_or_steal_keyboard_input() {
    let mut c = controller("");
    let bounds = Rect::new(0, 0, 80, 24);
    let p = ui::geometry(&c.app, bounds).help.unwrap().picker.unwrap();
    mouse(&mut c, bounds, p.query);
    mouse(&mut c, bounds, Rect::new(p.query.x, p.query.bottom(), 1, 1));
    assert!(c.app.help_picker.is_some());
    let (code, mods) = chord("t").unwrap();
    let g = ui::geometry(&c.app, bounds);
    c.handle(Event::Key(KeyEvent::new(code, mods)), &g, Duration::ZERO);
    assert_eq!(c.app.help_picker.as_ref().unwrap().query, "t");
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn command_details_share_the_primary_rows_mouse_target_and_keep_exact_keys() {
    let bounds = Rect::new(0, 0, 100, 32);
    let mut c = controller("mkdir");
    let p = ui::geometry(&c.app, bounds).help.unwrap().picker.unwrap();
    let (area, index) = p
        .controls
        .iter()
        .find(|(_, i)| matches!(p.hits[*i].target, HelpTarget::Action(_)))
        .copied()
        .unwrap();
    assert_eq!(area.height, 2);
    let target = p.hits[index].target;
    let b = draw(&c, bounds, ColorLevel::TrueColor);
    assert!(row(&b, area).contains("Create directory"));
    let details = Rect::new(area.x, area.y + 1, area.width, 1);
    assert!(row(&b, details).contains(&p.hits[index].keys));
    assert!(row(&b, details).contains("File Operations"));
    mouse(&mut c, bounds, details);
    assert!(c.app.help_picker.is_none());
    assert_eq!(c.app.help_landing.unwrap().0, target);
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn narrow_results_keep_complete_nonoverlapping_regions_and_selected_rows_visible() {
    use crossterm::event::KeyCode;
    for (width, height) in [(1, 1), (12, 8), (20, 12), (40, 16), (80, 24)] {
        let bounds = Rect::new(0, 0, width, height);
        let mut c = controller("pane");
        for _ in 0..12 {
            let g = ui::geometry(&c.app, bounds);
            c.handle(
                Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
                &g,
                Duration::ZERO,
            );
            let p = ui::geometry(&c.app, bounds).help.unwrap().picker.unwrap();
            draw(&c, bounds, ColorLevel::NoColor);
            assert!(
                p.controls
                    .windows(2)
                    .all(|pair| pair[0].0.bottom() <= pair[1].0.y)
            );
            for (area, _) in &p.controls {
                assert!(area.y >= p.shell.body.y && area.bottom() <= p.shell.body.bottom());
                assert!(area.right() <= bounds.right());
                assert!(area.y >= p.separator.bottom());
            }
            if p.shell.body.height >= 2 {
                assert!(
                    p.controls
                        .iter()
                        .any(|(_, i)| *i == c.app.help_picker.as_ref().unwrap().selected)
                );
            }
        }
    }
}

#[test]
fn long_unicode_queries_keep_the_label_cursor_and_query_style_outside_results() {
    for width in [12, 20, 40, 80] {
        let bounds = Rect::new(0, 0, width, 24);
        let c = controller(&"界e\u{301}".repeat(100));
        let p = ui::geometry(&c.app, bounds).help.unwrap().picker.unwrap();
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let cursor = terminal.get_cursor_position().unwrap();
        assert!(p.query.contains(cursor));
        let text = row(terminal.backend().buffer(), p.query);
        assert!(text.starts_with(if p.query.width >= 10 {
            "Search: "
        } else {
            "/ "
        }));
        for x in p.query.x..p.query.right() {
            assert!(
                !terminal.backend().buffer()[(x, p.query.y)]
                    .modifier
                    .contains(ratatui::style::Modifier::REVERSED)
            );
        }
    }
}

#[test]
fn keyboard_and_either_command_row_jump_to_the_same_anchor_without_running_it() {
    use crossterm::event::KeyCode;
    let bounds = Rect::new(0, 0, 80, 24);
    for click_row in [None, Some(0), Some(1)] {
        let mut c = controller("pane");
        let g = ui::geometry(&c.app, bounds);
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            &g,
            Duration::ZERO,
        );
        let p = ui::geometry(&c.app, bounds).help.unwrap().picker.unwrap();
        let target = p.hits[1].target;
        let area = p.controls.iter().find(|(_, index)| *index == 1).unwrap().0;
        assert_eq!(area.height, 2);
        if let Some(row) = click_row {
            mouse(
                &mut c,
                bounds,
                Rect::new(area.x, area.y + row, area.width, 1),
            );
        } else {
            let g = ui::geometry(&c.app, bounds);
            c.handle(
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                &g,
                Duration::ZERO,
            );
        }
        assert!(c.app.help_picker.is_none());
        assert_eq!(c.app.help_landing.unwrap().0, target);
        assert!(c.app.operations.outbox.is_empty());
        assert!(!c.quit);
    }
}
