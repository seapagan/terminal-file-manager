use super::*;
use colored_text::ColorLevel;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};
use terminal_file_manager::color::Theme;

pub(super) fn fixture(pane: usize, overlap: bool) -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta", "gamma"] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    let mut c = controller(root.path());
    c.app.dual = true;
    c.app.active = pane;
    for id in 0..2 {
        c.app.panes[id].cursor = Some(root.path().join("alpha"));
        c.app.panes[id].toggle_mark();
        c.app.panes[id].cursor = Some(root.path().join(if overlap { "alpha" } else { "beta" }));
    }
    (root, c)
}

pub(super) fn draw(c: &Controller, width: u16, theme: &Theme) -> (Buffer, ui::Geometry) {
    let mut t = Terminal::new(TestBackend::new(width, 24)).unwrap();
    let mut geometry = None;
    t.draw(|f| geometry = Some(ui::render(f, &c.app, theme)))
        .unwrap();
    (t.backend().buffer().clone(), geometry.unwrap())
}

pub(super) fn entry_row(c: &Controller, g: &ui::Geometry, pane: usize, name: &str) -> Rect {
    let p = &c.app.panes[pane];
    let row = p
        .visible
        .iter()
        .position(|i| p.entries[*i].name == name)
        .unwrap()
        + usize::from(p.has_parent());
    let area = g.panes[pane].unwrap();
    Rect::new(
        area.x + 1,
        area.y + 2 + (row - p.viewport_start(g.listing_rows[pane])) as u16,
        area.width.saturating_sub(2),
        1,
    )
}

fn assert_rows(
    c: &Controller,
    before: &(Buffer, ui::Geometry),
    width: u16,
    theme: &Theme,
    bright: &[&str],
) {
    let (after, g) = draw(c, width, theme);
    for pane in 0..2 {
        if g.panes[pane].is_none() {
            continue;
        }
        for name in ["alpha", "beta", "gamma"] {
            let old = entry_row(c, &before.1, pane, name);
            let new = entry_row(c, &g, pane, name);
            for offset in 0..new.width {
                let a = &after[(new.x + offset, new.y)];
                let b = &before.0[(old.x + offset, old.y)];
                if pane == c.app.active && bright.contains(&name) {
                    assert_eq!(a, b, "bright {pane} {name} {offset}");
                } else {
                    assert_eq!(a.symbol(), b.symbol());
                    assert_eq!(a.bg, b.bg);
                    assert_muted(a, theme.level);
                }
            }
        }
    }
}

pub(super) fn assert_muted(cell: &ratatui::buffer::Cell, level: ColorLevel) {
    let expected = match level {
        ColorLevel::NoColor => Color::Reset,
        ColorLevel::Ansi16 => Color::DarkGray,
        ColorLevel::Ansi256 => Color::Indexed(242),
        ColorLevel::TrueColor => Color::Rgb(110, 110, 110),
    };
    assert_eq!(cell.fg, expected);
    assert!(!cell.modifier.contains(Modifier::BOLD));
    assert_eq!(
        cell.modifier.contains(Modifier::DIM),
        level == ColorLevel::NoColor
    );
}

#[test]
fn captured_sources_dim_complete_rows_in_both_panes_across_widths_and_colours() {
    for pane in 0..2 {
        for overlap in [false, true] {
            for level in [
                ColorLevel::NoColor,
                ColorLevel::Ansi16,
                ColorLevel::Ansi256,
                ColorLevel::TrueColor,
            ] {
                for width in [20, 48, 100, 180] {
                    for source in [None, Some('m'), Some('h'), Some('c')] {
                        let (_root, mut c) = fixture(pane, overlap);
                        let theme = Theme::new(level);
                        let before = draw(&c, width, &theme);
                        let marks = c.app.panes.each_ref().map(|p| p.marks.clone());
                        let cursors = c.app.panes.each_ref().map(|p| p.cursor.clone());
                        send(&mut c, KeyCode::Char('y'));
                        if let Some(source) = source {
                            send(&mut c, KeyCode::Char(source));
                        }
                        let highlighted = if overlap { "alpha" } else { "beta" };
                        let bright = match source {
                            None => vec!["alpha", highlighted],
                            Some('m') => vec!["alpha"],
                            Some('h') => vec![highlighted],
                            _ => vec![],
                        };
                        assert_rows(&c, &before, width, &theme, &bright);
                        assert_eq!(c.app.panes.each_ref().map(|p| p.marks.clone()), marks);
                        assert_eq!(c.app.panes.each_ref().map(|p| p.cursor.clone()), cursors);
                        assert!(c.path_copy_request.is_none() && c.path_base_request.is_none());
                    }
                }
            }
        }
    }
}

pub(super) fn assert_normal(c: &Controller, before: &(Buffer, ui::Geometry), theme: &Theme) {
    let (after, g) = draw(c, 180, theme);
    for pane in 0..2 {
        for name in ["alpha", "beta", "gamma"] {
            let old = entry_row(c, &before.1, pane, name);
            let new = entry_row(c, &g, pane, name);
            for x in 0..new.width {
                assert_eq!(after[(new.x + x, new.y)], before.0[(old.x + x, old.y)]);
            }
        }
    }
}

#[test]
fn cancel_completion_and_base_choices_restore_original_rows_without_reset_state() {
    let theme = Theme::new(ColorLevel::TrueColor);
    for pane in 0..2 {
        for keys in ["y", "ym", "yh", "ymn", "yhn", "ycn", "ys", "yS"] {
            let (root, mut c) = fixture(pane, false);
            let before = draw(&c, 180, &theme);
            let marks = c.app.panes.each_ref().map(|p| p.marks.clone());
            sequence(&mut c, keys);
            if c.app.footer_workflow.is_some() {
                send(&mut c, KeyCode::Esc);
            }
            assert!(c.app.footer_workflow.is_none());
            assert_normal(&c, &before, &theme);
            assert_eq!(c.app.active, pane);
            assert_eq!(c.app.relative_path_base, root.path());
            assert_eq!(c.app.panes.each_ref().map(|p| p.marks.clone()), marks);
            assert!(!c.preferences_changed && c.app.operations.outbox.is_empty());
            if let Some(request) = c.path_copy_request.take() {
                let expected = match keys {
                    "ymn" => "alpha",
                    "yhn" => "beta",
                    _ => root.path().file_name().unwrap().to_str().unwrap(),
                };
                assert_eq!(request.prepare().unwrap(), expected);
            }
        }
    }
}

#[test]
fn help_preserves_focus_and_backspace_restores_the_initial_source_union() {
    for pane in 0..2 {
        let (_root, mut c) = fixture(pane, false);
        let theme = Theme::new(ColorLevel::NoColor);
        let before = draw(&c, 180, &theme);
        sequence(&mut c, "yh");
        let focused = draw(&c, 180, &theme);
        send(&mut c, KeyCode::F(1));
        let (help, g) = draw(&c, 180, &theme);
        let shell = g.context_help.unwrap().shell.outer;
        let mut checked = 0;
        for id in 0..2 {
            for name in ["alpha", "beta", "gamma"] {
                let area = entry_row(&c, &focused.1, id, name);
                for x in area.x..area.right() {
                    if !shell.contains((x, area.y).into()) {
                        assert_eq!(help[(x, area.y)], focused.0[(x, area.y)]);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0);
        send(&mut c, KeyCode::Esc);
        assert_eq!(draw(&c, 180, &theme).0, focused.0);
        send(&mut c, KeyCode::Backspace);
        assert_rows(&c, &before, 180, &theme, &["alpha", "beta"]);
        send(&mut c, KeyCode::Esc);
        assert_normal(&c, &before, &theme);
    }
}

#[test]
fn invalid_origin_context_stops_dimming_before_any_reset_or_input_event() {
    let (_root, mut c) = fixture(0, false);
    let theme = Theme::new(ColorLevel::TrueColor);
    let before = draw(&c, 180, &theme);
    sequence(&mut c, "ym");
    c.app.panes[0].pending = Some(c.app.panes[0].local_path().clone());
    assert_normal(&c, &before, &theme);
    assert!(c.app.footer_workflow.is_some());
    assert!(c.reconcile_footer());
    assert_normal(&c, &before, &theme);
}

pub(super) fn assert_footer(after: &Buffer, layout: &ui::workflow::Layout, theme: &Theme) {
    let mut terminal = Terminal::new(TestBackend::new(180, 24)).unwrap();
    terminal
        .draw(|frame| ui::workflow::render(frame, layout, theme))
        .unwrap();
    for control in &layout.controls {
        for x in control.area.x..control.area.right() {
            assert_eq!(
                after[(x, control.area.y)],
                terminal.backend().buffer()[(x, control.area.y)]
            );
        }
    }
}

#[test]
fn neutral_grey_on_vte_preserves_bright_rows_and_uses_faint_only_without_colour() {
    use ratatui::backend::{Backend, CrosstermBackend};
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let (_root, mut c) = fixture(0, false);
        let theme = Theme::with_vte_version(level, Some("7800"));
        let before = draw(&c, 180, &theme);
        sequence(&mut c, "ym");
        assert_rows(&c, &before, 180, &theme, &["alpha"]);
        let (after, g) = draw(&c, 180, &theme);
        let area = entry_row(&c, &g, 0, "gamma");
        for x in area.x..area.right() {
            assert_muted(&after[(x, area.y)], level);
        }
        let mut bytes = Vec::new();
        CrosstermBackend::new(&mut bytes)
            .draw((area.x..area.right()).map(|x| (x, area.y, &after[(x, area.y)])))
            .unwrap();
        let output = String::from_utf8(bytes).unwrap();
        assert_eq!(
            output.contains("\x1b[2m"),
            level == ColorLevel::NoColor,
            "{output:?}"
        );
        assert!(output.ends_with("\x1b[0m"));
    }
}

#[test]
fn mouse_source_and_cancel_use_the_same_focus_and_restore_the_listing() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use terminal_file_manager::workflow::Target;
    let (_root, mut c) = fixture(1, false);
    let theme = Theme::new(ColorLevel::Ansi16);
    let before = draw(&c, 180, &theme);
    sequence(&mut c, "y");
    for target in [Target::Choice("marked"), Target::Cancel] {
        let g = ui::geometry(&c.app, Rect::new(0, 0, 180, 24));
        let area = g
            .workflow
            .as_ref()
            .unwrap()
            .controls
            .iter()
            .find(|control| control.target == Some(target))
            .unwrap()
            .area;
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: area.x,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            Default::default(),
        );
        if target == Target::Choice("marked") {
            assert_rows(&c, &before, 180, &theme, &["alpha"]);
        }
    }
    assert!(c.app.footer_workflow.is_none());
    assert_normal(&c, &before, &theme);
}

#[test]
fn parent_navigation_is_not_a_highlighted_entry_and_scrolled_rows_align() {
    let (_root, mut c) = fixture(0, false);
    let theme = Theme::new(ColorLevel::NoColor);
    c.app.panes[0].cursor = None;
    sequence(&mut c, "y");
    let (buffer, g) = draw(&c, 180, &theme);
    let area = g.panes[0].unwrap();
    assert!(
        buffer[(area.x + 1, area.y + 2)]
            .modifier
            .contains(Modifier::DIM)
    );
    let marked = entry_row(&c, &g, 0, "alpha");
    assert!(
        !buffer[(marked.x, marked.y)]
            .modifier
            .contains(Modifier::DIM)
    );
    send(&mut c, KeyCode::Esc);
    c.app.panes[0].cursor = Some(c.app.panes[0].local_path().join("beta"));
    c.app.panes[0].scroll = 2;
    sequence(&mut c, "yh");
    let (buffer, g) = draw(&c, 180, &theme);
    for (name, dim) in [("beta", false), ("gamma", true)] {
        let row = entry_row(&c, &g, 0, name);
        assert_eq!(buffer[(row.x, row.y)].modifier.contains(Modifier::DIM), dim);
    }
}
