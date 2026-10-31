use super::{actions::*, *};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn presets_start_safe_and_destructive_focus_cannot_convert_enter_into_approval() {
    for preset in [
        Preset::OkCancel,
        Preset::YesNo,
        Preset::ConfirmCancel,
        Preset::Destructive {
            label: "D Delete",
            key: KeyCode::Char('D'),
        },
    ] {
        let mut group = preset.group();
        assert_eq!(group.key(press(KeyCode::Enter)), Some(Response::Cancel));
        group.key(press(KeyCode::Left));
        if matches!(preset, Preset::Destructive { .. }) {
            assert_ne!(group.key(press(KeyCode::Enter)), Some(Response::Accept));
            assert_eq!(group.key(press(KeyCode::Char('D'))), Some(Response::Accept));
            assert_eq!(group.key(press(KeyCode::Char(' '))), None);
        } else {
            assert_eq!(group.key(press(KeyCode::Enter)), Some(Response::Accept));
        }
        assert_eq!(group.key(press(KeyCode::Esc)), Some(Response::Cancel));
    }
    assert_eq!(
        Preset::Ok.group().key(press(KeyCode::Enter)),
        Some(Response::Accept)
    );
}

#[test]
fn disabled_actions_and_nonpress_events_never_produce_an_intent() {
    let mut group = Preset::YesNo.group();
    group.actions[0].enabled = false;
    group.key(press(KeyCode::Left));
    assert_eq!(group.selected, 1);
    assert_eq!(group.key(press(KeyCode::Char('y'))), None);
    for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
        assert_eq!(
            group.key(KeyEvent::new_with_kind(
                KeyCode::Enter,
                KeyModifiers::NONE,
                kind
            )),
            None
        );
    }
    let layout = Spec {
        title: "Question",
        help: false,
        content_width: 4,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &group.actions,
    }
    .measure(Rect::new(0, 0, 80, 25), |_| 1);
    let disabled = &layout.actions[0];
    assert_eq!(layout.hit(disabled.area.x, disabled.area.y), None);
    let cancel = &layout.actions[1];
    assert_eq!(
        layout.hit(cancel.area.x, cancel.area.y),
        Some(&Response::Cancel)
    );
}

#[test]
fn deliberate_uppercase_shortcuts_accept_shift_but_not_control_or_alt() {
    let mut group = Preset::Destructive {
        label: "D Delete",
        key: KeyCode::Char('D'),
    }
    .group();
    assert_eq!(
        group.key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)),
        Some(Response::Accept)
    );
    for modifiers in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
        assert_eq!(
            group.key(KeyEvent::new(KeyCode::Char('D'), modifiers)),
            None
        );
    }
}

#[test]
fn narrow_preset_retains_the_original_safe_action_focus_identity() {
    let group = Preset::Destructive {
        label: "D DELETE permanently",
        key: KeyCode::Char('D'),
    }
    .group();
    let layout = Spec {
        title: "Delete",
        help: false,
        content_width: 10,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &group.actions,
    }
    .measure(Rect::new(0, 0, 20, 20), |_| 1);
    assert_eq!(layout.actions.len(), 1);
    assert_eq!(layout.actions[0].action.intent, Response::Cancel);
    let theme = Theme::new(colored_text::ColorLevel::NoColor);
    let mut terminal = Terminal::new(TestBackend::new(20, 20)).unwrap();
    terminal
        .draw(|frame| actions::render(frame, &layout, &theme, Some(group.selected)))
        .unwrap();
    let area = layout.actions[0].area;
    assert!(
        terminal.backend().buffer()[(area.x, area.y)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED)
    );
}

#[test]
fn intrinsic_shell_has_padding_and_exact_centred_unicode_hit_regions() {
    let actions = [
        Action::custom("界e\u{301}", 7, KeyCode::Char('x')),
        Action::custom("OK", 8, KeyCode::Enter),
    ];
    for width in [20, 40, 80, 120] {
        let layout = Spec {
            title: "Title",
            help: false,
            content_width: 12,
            pinned_rows: 0,
            footer_rows: 0,
            actions: &actions,
        }
        .measure(Rect::new(0, 0, width, 25), |_| 2);
        assert_eq!(layout.outer.width, 16);
        assert_eq!(layout.body.x - layout.outer.x, 2);
        assert_eq!(layout.body.y - layout.outer.y, 2);
        assert_eq!(layout.body.height, 2);
        assert_eq!(layout.scroll_limit(12), 10);
        let left = layout.actions[0].area.x - layout.outer.x;
        let right = layout.outer.right() - layout.actions[1].area.right();
        assert!(left.abs_diff(right) <= 1);
        assert_action_cells(&layout, width);
    }
}

fn assert_action_cells(layout: &Layout<i32>, width: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, 25)).unwrap();
    terminal
        .draw(|frame| {
            render_shell(
                frame,
                layout.outer,
                "Title",
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
            actions::render(
                frame,
                layout,
                &Theme::new(colored_text::ColorLevel::NoColor),
                Some(0),
            );
        })
        .unwrap();
    for region in &layout.actions {
        assert_eq!(
            terminal.backend().buffer()[(region.area.x, region.area.y)].symbol(),
            "["
        );
        assert_eq!(
            terminal.backend().buffer()[(region.area.right() - 1, region.area.y)].symbol(),
            "]"
        );
        assert_eq!(
            layout.hit(region.area.right() - 1, region.area.y),
            Some(&region.action.intent)
        );
        assert_eq!(layout.hit(region.area.right(), region.area.y), None);
    }
}

#[test]
fn long_content_and_titles_obey_tiny_offset_bounds_and_keep_actions_fixed() {
    let group = Preset::Ok.group();
    for width in [0, 1, 2, 4, 20, 40, 80, 120] {
        for height in [0, 1, 2, 5, 8, 30] {
            let bounds = Rect::new(3, 5, width, height);
            let layout = Spec {
                title: &"界".repeat(500),
                help: false,
                content_width: 10000,
                pinned_rows: 5,
                footer_rows: 0,
                actions: &group.actions,
            }
            .measure(bounds, |_| 1000);
            assert_eq!(layout.outer.intersection(bounds), layout.outer);
            for action in &layout.actions {
                assert_eq!(action.area.intersection(layout.outer), action.area);
                assert!(action.area.y >= layout.body.bottom());
            }
            assert!(layout.scroll_limit(1000) >= 980);
        }
    }
}

#[test]
fn file_rows_keep_units_whole_and_truncate_only_on_grapheme_boundaries() {
    for width in [0, 1, 4, 20, 40, 80, 120] {
        let row = file_row("界e\u{301}".repeat(100).as_str(), "1023.9 KiB", width);
        assert!(super::width(&row) <= width);
        if width >= 20 {
            assert!(row.ends_with("1023.9 KiB"));
        }
    }
    assert_eq!(truncate("界e\u{301}abc", 4), "界e\u{301}…");
    assert_eq!(file_size(Some(1024)), "1 KiB");
    assert!(!file_size(Some(u64::MAX)).contains("Some"));
    assert_eq!(
        super::width(&truncate(&"x".repeat(70000), u16::MAX)),
        u16::MAX
    );
}

#[test]
fn file_row_fits_its_exact_measured_width_with_one_cell_gap() {
    let row = Row::<usize>::file("界e\u{301}.txt".into(), "1 KiB".into());
    let required = Line::raw(&row.text).width() + 1 + Line::raw("1 KiB").width();
    assert_eq!(row.width(), required);
    let theme = Theme::new(colored_text::ColorLevel::NoColor);
    assert_eq!(
        row.line(required as u16, &theme).to_string(),
        "界e\u{301}.txt 1 KiB"
    );
    let clipped = row.line(required as u16 - 1, &theme).to_string();
    assert!(clipped.starts_with("界e\u{301}"));
    assert!(clipped.ends_with("… 1 KiB"));
}

#[test]
fn structured_content_escapes_controls_and_bidi_before_measuring_or_rendering() {
    let row = Row::<usize>::file("link/界e\u{301}\n\u{202e}\u{1b}".into(), "10 KiB".into());
    let wrapped = row.wrap(40);
    assert_eq!(wrapped.len(), 1);
    let line = wrapped[0].line(40, &Theme::new(colored_text::ColorLevel::NoColor));
    assert_eq!(line.width(), 40);
    assert!(line.to_string().contains("\\n\\u{202e}\\u{1b}"));
    assert!(line.to_string().ends_with("10 KiB"));
    assert!(!line.to_string().contains('\u{1b}'));
}

#[test]
fn long_escaped_field_keeps_the_editing_end_and_caret_visible() {
    let value = format!("{}end", "\u{1b}".repeat(16384));
    let row = Row::field("Name", &value, 0usize, true).wrap(40).remove(0);
    let line = row.line(40, &Theme::new(colored_text::ColorLevel::NoColor));
    assert!(line.width() <= 40);
    assert!(line.to_string().ends_with("end▏"));
    assert!(!line.to_string().contains('\u{1b}'));
}

#[test]
fn common_roots_are_lexical_and_unrelated_roots_remain_uncompressed() {
    let a = Path::new("/work/link/../one/a");
    let b = Path::new("/work/link/../one/b");
    assert_eq!(
        common_root([a, b]),
        Some(PathBuf::from("/work/link/../one"))
    );
    assert_eq!(
        common_root([Path::new("/a/file"), Path::new("/b/file")]),
        None
    );
    assert_eq!(common_root(std::iter::empty::<&Path>()), None);
}

#[cfg(windows)]
#[test]
fn windows_drive_and_unc_roots_do_not_compress_unrelated_locations() {
    assert_eq!(
        common_root([Path::new(r"C:\work\a"), Path::new(r"D:\work\b")]),
        None
    );
    assert_eq!(
        common_root([
            Path::new(r"\\server\share\a"),
            Path::new(r"\\server\other\b")
        ]),
        None
    );
    assert_eq!(
        common_root([
            Path::new(r"\\server\share\work\a"),
            Path::new(r"\\server\share\work\b")
        ]),
        Some(PathBuf::from(r"\\server\share\work"))
    );
}

use std::path::{Path, PathBuf};

#[test]
fn capped_shell_centres_in_the_original_usable_area() {
    let actions = Preset::Ok.group();
    for width in [20, 40, 80, 120] {
        for height in [16, 25, 40, 60] {
            for count in [1, 100] {
                let bounds = Rect::new(3, 5, width, height);
                let layout = Spec {
                    title: "Centred",
                    help: false,
                    content_width: 30,
                    pinned_rows: 0,
                    footer_rows: 0,
                    actions: &actions.actions,
                }
                .measure(bounds, |_| count);
                let top = layout.outer.y - bounds.y;
                let bottom = bounds.bottom() - 2 - layout.outer.bottom();
                assert!(top.abs_diff(bottom) <= 1, "{bounds:?} {layout:?}");
                assert!(layout.outer.height <= height * 3 / 4);
            }
        }
    }
}

#[test]
fn shared_shell_insets_all_titles_one_border_cell_and_keeps_border_style() {
    for title in [
        "Copy",
        "Move",
        "Move to Trash",
        "PERMANENT DELETE — irreversible",
        "Jobs / progress / results",
        "Metadata",
        "File already exists",
        "界e\u{301}long title",
    ] {
        for width in [4, 12, 40, 80] {
            let mut terminal = Terminal::new(TestBackend::new(width, 10)).unwrap();
            let theme = Theme::new(colored_text::ColorLevel::Ansi16);
            terminal
                .draw(|frame| {
                    render_shell_with_title_role(
                        frame,
                        Rect::new(0, 0, width, 10),
                        title,
                        &theme,
                        Role::Error,
                    )
                })
                .unwrap();
            let b = terminal.backend().buffer();
            assert_eq!(b[(0, 0)].symbol(), "┌");
            assert_eq!(b[(1, 0)].symbol(), "─");
            let shown = truncate(title, width - 3);
            let mut x = 2;
            for grapheme in Line::raw(&shown).styled_graphemes(ratatui::style::Style::default()) {
                assert_eq!(b[(x, 0)].symbol(), grapheme.symbol);
                x += super::width(grapheme.symbol);
            }
            assert_eq!(b[(width - 1, 0)].symbol(), "┐");
            assert_eq!(b[(1, 0)].fg, theme.style(Role::Header).fg.unwrap());
            assert_eq!(b[(2, 0)].fg, theme.style(Role::Error).fg.unwrap());
        }
    }
}

#[test]
fn optional_help_header_reserves_unicode_title_and_exact_cells_at_all_widths() {
    for title in ["Pattern selection", "界 λ Help"] {
        for width in [1, 20, 40, 80, 120] {
            for level in [
                colored_text::ColorLevel::NoColor,
                colored_text::ColorLevel::Ansi16,
                colored_text::ColorLevel::Ansi256,
                colored_text::ColorLevel::TrueColor,
            ] {
                assert_help_header(title, width, level);
            }
        }
    }
}

fn assert_help_header(title: &str, width: u16, level: colored_text::ColorLevel) {
    let theme = Theme::new(level);
    let spec = Spec::<()> {
        title,
        help: true,
        content_width: 50,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    };
    let bounds = Rect::new(0, 0, width, 20);
    let layout = spec.measure(bounds, |_| 2);
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
    terminal
        .draw(|f| render_measured(f, &layout, title, &theme, Role::Header))
        .unwrap();
    if let Some(area) = layout.help {
        let buffer = terminal.backend().buffer();
        let text: String = (area.x..area.right())
            .map(|x| buffer[(x, area.y)].symbol())
            .collect();
        assert_eq!(text, "[? F1]");
        assert!(
            buffer[(area.x, area.y)]
                .modifier
                .contains(ratatui::style::Modifier::UNDERLINED)
        );
        assert!(area.x >= layout.outer.x + 2 + super::width(title));
        assert!(area.right() < layout.outer.right());
    } else {
        assert!(super::width(title).saturating_add(11) > layout.outer.width);
    }
    assert!(
        Spec {
            help: false,
            ..spec
        }
        .measure(bounds, |_| 2)
        .help
        .is_none()
    );
}
