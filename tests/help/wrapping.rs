use super::*;
use ratatui::buffer::Buffer;
use terminal_file_manager::{
    color::Role,
    input::{Action, HelpEntry, HelpTarget},
};

const DESCRIPTION: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";

const WORD_CASES: &[(u16, &[&str])] = &[
    (
        24,
        &[
            "alpha beta gamma",
            "delta epsilon",
            "zeta eta theta",
            "iota kappa lambda",
            "mu",
        ],
    ),
    (
        40,
        &[
            "alpha beta gamma delta epsilon",
            "zeta eta theta iota kappa lambda",
            "mu",
        ],
    ),
    (
        60,
        &[
            "alpha beta gamma delta epsilon zeta eta theta iota",
            "kappa lambda mu",
        ],
    ),
    (80, &[DESCRIPTION]),
    (120, &[DESCRIPTION]),
];

fn fixture(description: &'static str) -> Controller {
    let mut c = controller();
    c.app.help = true;
    c.app.help_rows = vec![
        HelpEntry {
            action: Action::Copy,
            keys: "F5".into(),
            description,
        },
        HelpEntry {
            action: Action::Move,
            keys: "F6".into(),
            description: "Move files to another folder",
        },
    ];
    c
}

fn anchor(layout: &ui::HelpLayout, action: Action) -> u16 {
    layout
        .anchors
        .iter()
        .find(|(target, _)| *target == HelpTarget::Action(action))
        .unwrap()
        .1
}

fn draw_at(c: &Controller, bounds: Rect, theme: &Theme) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    let mut geometry = ui::geometry(&c.app, bounds);
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, theme))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

fn row(buffer: &Buffer, area: Rect, y: u16) -> String {
    (area.x..area.right())
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .into()
}

fn event_at(c: &mut Controller, event: Event, bounds: Rect) {
    let geometry = ui::geometry(&c.app, bounds);
    assert!(c.handle(event, &geometry, Duration::ZERO).is_empty());
}

fn press(c: &mut Controller, code: KeyCode, bounds: Rect) {
    event_at(
        c,
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        bounds,
    );
}

#[test]
fn rendered_descriptions_wrap_at_words_and_keep_columns_styles_and_measured_anchors() {
    for &(width, expected) in WORD_CASES {
        let bounds = Rect::new(0, 0, width, 10);
        let mut c = fixture(DESCRIPTION);
        let layout = ui::geometry(&c.app, bounds).help.unwrap();
        let start = anchor(&layout, Action::Copy);
        assert_eq!(anchor(&layout, Action::Move) - start, expected.len() as u16);
        c.app.overlay_scroll = start;
        for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
            let theme = Theme::new(level);
            let (buffer, g) = draw_at(&c, bounds, &theme);
            let body = g.help.unwrap().body;
            let description = Rect::new(body.x + 3, body.y, body.width - 3, body.height);
            for (offset, text) in expected.iter().enumerate() {
                let y = body.y + offset as u16;
                assert_eq!(row(&buffer, description, y), *text, "{width} columns");
                assert_eq!(
                    buffer[(description.x, y)].fg,
                    theme.style(Role::Value).fg.unwrap_or_default()
                );
                assert_eq!(
                    row(&buffer, Rect::new(body.x, y, 2, 1), y),
                    if offset == 0 { "F5" } else { "" }
                );
            }
            assert_eq!(
                buffer[(body.x, body.y)].fg,
                theme.style(Role::Key).fg.unwrap_or_default()
            );
        }
    }
}

#[test]
fn rendered_long_unicode_words_split_without_losing_graphemes_or_text_while_scrolling() {
    const TEXT: &str = "界e\u{301} 界界界界界界界界界界🙂abcdefghijklmno end";
    for width in [12, 24, 40, 80] {
        let bounds = Rect::new(0, 0, width, 10);
        let mut c = fixture(TEXT);
        let layout = ui::geometry(&c.app, bounds).help.unwrap();
        let start = anchor(&layout, Action::Copy);
        let mut shown = String::new();
        for offset in 0..anchor(&layout, Action::Move) - start {
            c.app.overlay_scroll = start + offset;
            assert!(c.app.overlay_scroll <= layout.scroll_limit);
            let (buffer, g) = draw_at(&c, bounds, &Theme::new(ColorLevel::NoColor));
            let body = g.help.unwrap().body;
            let text = row(
                &buffer,
                Rect::new(body.x + 3, body.y, body.width - 3, 1),
                body.y,
            );
            if offset == 0 {
                assert!(text.starts_with("界 e\u{301}") || text.starts_with("界e\u{301}"));
            }
            shown.extend(text.chars().filter(|c| !c.is_whitespace()));
        }
        assert_eq!(
            shown,
            TEXT.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>(),
            "{width} columns"
        );
    }
}

#[test]
fn keyboard_and_mouse_search_jumps_use_the_word_wrapped_action_anchor() {
    for width in [24, 40, 80] {
        for mouse in [false, true] {
            let bounds = Rect::new(0, 0, width, 20);
            let mut c = fixture(DESCRIPTION);
            let layout = ui::geometry(&c.app, bounds).help.unwrap();
            let target = anchor(&layout, Action::Move);
            press(&mut c, KeyCode::Char('/'), bounds);
            event_at(&mut c, Event::Paste("F6".into()), bounds);
            if mouse {
                click_move_hit(&mut c, bounds);
            } else {
                press(&mut c, KeyCode::Enter, bounds);
            }
            assert_eq!(c.app.overlay_scroll, target.min(layout.scroll_limit));
            assert!(c.app.help && c.app.help_picker.is_none());
            assert!(c.app.operations.dialog.is_none() && c.app.operations.outbox.is_empty());
            let (buffer, g) = draw_at(&c, bounds, &Theme::new(ColorLevel::NoColor));
            let body = g.help.unwrap().body;
            let y = body.y + target - c.app.overlay_scroll;
            assert!(
                row(&buffer, Rect::new(body.x + 3, y, body.width - 3, 1), y)
                    .starts_with("Move files")
            );
        }
    }
}

fn click_move_hit(c: &mut Controller, bounds: Rect) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let g = ui::geometry(&c.app, bounds);
    let picker = g.help.unwrap().picker.unwrap();
    let (area, index) = picker.controls[0];
    assert_eq!(picker.hits[index].target, HelpTarget::Action(Action::Move));
    event_at(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        bounds,
    );
}
