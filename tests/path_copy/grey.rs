use super::{
    focus::{assert_footer, assert_muted, draw, entry_row, fixture},
    *,
};
use colored_text::ColorLevel;
use ratatui::{buffer::Buffer, style::Modifier};
use terminal_file_manager::color::Theme;

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::NoColor,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

fn assert_area(after: &Buffer, before: &Buffer, area: Rect, level: ColorLevel) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let a = &after[(x, y)];
            let b = &before[(x, y)];
            assert_eq!(a.bg, b.bg, "background {x},{y}");
            assert_eq!(a.symbol(), b.symbol());
            assert_muted(a, level);
        }
    }
}

fn assert_browser(c: &Controller, before: &(Buffer, ui::Geometry), theme: &Theme, bright: &[&str]) {
    let (after, g) = draw(c, 180, theme);
    assert_eq!(g.panes, before.1.panes);
    for pane in 0..2 {
        let Some(area) = g.panes[pane] else {
            continue;
        };
        let bright: Vec<_> = bright
            .iter()
            .filter(|_| pane == c.app.active)
            .map(|name| entry_row(c, &g, pane, name))
            .collect();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if bright.iter().any(|r| r.contains((x, y).into())) {
                    assert_eq!(after[(x, y)], before.0[(x, y)]);
                } else {
                    assert_area(&after, &before.0, Rect::new(x, y, 1, 1), theme.level);
                }
            }
        }
    }
    for area in [Some(g.status), g.sidebar, g.divider].into_iter().flatten() {
        assert_area(&after, &before.0, area, theme.level);
    }
    assert_footer(&after, g.workflow.as_ref().unwrap(), theme);
}

#[test]
fn complete_browser_is_neutral_with_original_backgrounds_and_selected_source_rows() {
    for level in LEVELS {
        for pane in 0..2 {
            for (keys, bright) in [
                ("y", &["alpha", "beta"][..]),
                ("ym", &["alpha"][..]),
                ("yh", &["beta"][..]),
                ("yc", &[][..]),
            ] {
                let (_root, mut c) = fixture(pane, false);
                for state in &mut c.app.panes {
                    state
                        .entries
                        .iter_mut()
                        .find(|e| e.name == "gamma")
                        .unwrap()
                        .error = Some("metadata unavailable".into());
                    state
                        .warnings
                        .push("Directory warning remains legible".into());
                }
                c.app.panes[1 - pane].error = Some("Filesystem error remains legible".into());
                let theme = Theme::new(level);
                let before = draw(&c, 180, &theme);
                sequence(&mut c, keys);
                assert_browser(&c, &before, &theme, bright);
            }
        }
    }
}

#[test]
fn sidebar_and_browser_status_are_muted_but_the_active_footer_is_unchanged() {
    for level in LEVELS {
        let (_root, mut c) = fixture(0, false);
        c.app.dual = false;
        c.app.sidebar = true;
        c.app.panes[0].sidebar = true;
        let theme = Theme::new(level);
        let before = draw(&c, 180, &theme);
        assert!(before.1.sidebar.is_some());
        sequence(&mut c, "yh");
        assert_browser(&c, &before, &theme, &["beta"]);
    }
}

#[test]
fn all_pane_regions_restore_after_exit() {
    for level in LEVELS {
        for keys in ["y", "ym", "yh", "ymn", "yhn", "ycn", "ys", "yS"] {
            let (_root, mut c) = fixture(0, false);
            let theme = Theme::new(level);
            let before = draw(&c, 180, &theme);
            sequence(&mut c, keys);
            if c.app.footer_workflow.is_some() {
                send(&mut c, KeyCode::Esc);
            }
            let (after, g) = draw(&c, 180, &theme);
            for area in g.panes.into_iter().flatten() {
                for y in area.y..area.bottom() {
                    for x in area.x..area.right() {
                        assert_eq!(after[(x, y)], before.0[(x, y)]);
                    }
                }
            }
            assert!(
                !after[(g.status.x, g.status.y)]
                    .modifier
                    .contains(Modifier::DIM)
            );
        }
    }
}

#[test]
fn invalid_context_restores_browser_before_reconciliation_or_another_input() {
    for level in LEVELS {
        let (_root, mut c) = fixture(0, false);
        let theme = Theme::new(level);
        sequence(&mut c, "ym");
        c.app.panes[0].pending = Some(c.app.panes[0].path.clone());
        let workflow = c.app.footer_workflow.take();
        let (normal, normal_g) = draw(&c, 180, &theme);
        c.app.footer_workflow = workflow;
        let (actual, g) = draw(&c, 180, &theme);
        assert_eq!(g.panes, normal_g.panes);
        for area in g.panes.into_iter().flatten().chain([g.status]) {
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    assert_eq!(actual[(x, y)], normal[(x, y)]);
                }
            }
        }
        assert!(c.reconcile_footer());
        assert_eq!(draw(&c, 180, &theme).0, normal);
    }
}
