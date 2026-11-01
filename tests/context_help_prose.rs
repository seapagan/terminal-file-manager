//! Context prose wraps words and retains semantic shortcut styles.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Modifier};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Action, Bindings, ContextHelp, Controller, HelpTopic},
    operations::{Operation, Plan},
    ui,
};
mod dialog_fixture;

fn controller(bindings: &std::collections::BTreeMap<String, Action>) -> Controller {
    let mut app = dialog_fixture::dialog(Plan::create_file("/controlled".into()));
    app.context_help = Some(ContextHelp {
        topic: HelpTopic::Operation(Operation::CreateFile),
        scroll: 0,
    });
    Controller::new(app, Bindings::new(bindings).unwrap(), Default::default())
}
fn draw(app: &App, bounds: Rect, level: ColorLevel) -> (Buffer, Rect) {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        ui::geometry(app, bounds).context_help.unwrap().shell.body,
    )
}
fn row(b: &Buffer, area: Rect, y: u16) -> String {
    (area.x..area.right()).map(|x| b[(x, y)].symbol()).collect()
}
fn find(b: &Buffer, area: Rect, text: &str) -> (u16, u16) {
    (area.y..area.bottom())
        .find_map(|y| {
            let line = row(b, area, y);
            line.find(text).map(|x| {
                (
                    area.x + ratatui::text::Line::raw(&line[..x]).width() as u16,
                    y,
                )
            })
        })
        .unwrap_or_else(|| panic!("missing {text}"))
}
#[test]
fn prose_moves_whole_words_to_the_next_line() {
    let c = controller(&Default::default());
    let (b, body) = draw(&c.app, Rect::new(0, 0, 40, 30), ColorLevel::NoColor);
    assert_eq!(
        row(&b, body, body.y).trim(),
        "Create an empty file or directory"
    );
    assert!(row(&b, body, body.y + 1).starts_with("in the shown parent."));
}
#[test]
fn actual_shortcuts_are_styled_and_survive_configured_unicode_and_backticks() {
    let c = controller(&std::collections::BTreeMap::from([
        ("Ctrl+`".into(), Action::CreateFile),
        ("Alt+界".into(), Action::CreateFile),
    ]));
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        let (b, body) = draw(&c.app, Rect::new(0, 0, 100, 80), level);
        for key in ["Ctrl+a", "Backspace", "Enter", "Esc", "Ctrl+`", "Alt+界"] {
            let (x, y) = find(&b, body, key);
            assert_eq!(b[(x, y)].fg, theme.style(Role::Key).fg.unwrap_or_default());
            assert!(b[(x, y)].modifier.contains(Modifier::BOLD));
        }
        let (x, y) = find(&b, body, "Create");
        assert!(!b[(x, y)].modifier.contains(Modifier::BOLD));
    }
}
#[test]
fn destructive_and_pattern_help_keep_their_case_sensitive_safety_instructions() {
    let cases = [
        (
            HelpTopic::Operation(Operation::Delete),
            [
                "Permanent deletion cannot be undone.",
                "Only uppercase D confirms permanent deletion.",
                "Lowercase d does not confirm.",
            ],
        ),
        (
            HelpTopic::Operation(Operation::Trash),
            [
                "Only uppercase T confirms the move to Trash.",
                "Lowercase t does not confirm.",
                "Enter and Esc cancel.",
            ],
        ),
        (
            HelpTopic::Pattern,
            [
                "? matches one byte.",
                "press Enter again to apply it.",
                "Correct invalid or overlong expressions before applying.",
            ],
        ),
        (
            HelpTopic::Incident,
            [
                "Type ACKNOWLEDGE exactly",
                "press F10 when enabled.",
                "Enter and Esc keep tfm open.",
            ],
        ),
    ];
    for (topic, needles) in cases {
        assert_safety_text(topic, needles);
    }
}

fn assert_safety_text(topic: HelpTopic, needles: [&str; 3]) {
    let mut c = controller(&Default::default());
    c.app.context_help.as_mut().unwrap().topic = topic;
    let (b, body) = draw(&c.app, Rect::new(0, 0, 100, 160), ColorLevel::NoColor);
    let text = (body.y..body.bottom())
        .map(|y| row(&b, body, y))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for needle in needles {
        assert!(text.contains(needle), "{needle}: {text}");
    }
}
