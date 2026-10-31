//! Landing feedback and line counts describe visible Help content.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, ContextHelp, Controller, HelpSection, HelpTarget, HelpTopic},
    ui,
};

fn draw(app: &App, level: ColorLevel) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();
    let mut geometry = None;
    terminal
        .draw(|f| geometry = Some(ui::render(f, app, &Theme::new(level))))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry.unwrap())
}

#[test]
fn section_landing_highlights_the_heading_without_highlighting_its_separator() {
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        for section in HelpSection::ALL.into_iter().skip(1) {
            let mut app = App::new("owned-help-landing".into());
            app.help = true;
            let target = HelpTarget::Section(section);
            let layout = ui::geometry(&app, Rect::new(0, 0, 100, 12)).help.unwrap();
            let anchor = layout.anchors.iter().find(|(t, _)| *t == target).unwrap().1;
            app.overlay_scroll = anchor - 1;
            let (before, _) = draw(&app, level);
            app.help_landing = Some((target, app.now + Duration::from_secs(2)));
            let (after, _) = draw(&app, level);
            let body = layout.body;
            for x in body.x..body.right() {
                assert_eq!(before[(x, body.y)], after[(x, body.y)], "{section:?}");
            }
            assert_ne!(before[(body.x, body.y + 1)], after[(body.x, body.y + 1)]);
        }
    }
}

fn line_range(buffer: &Buffer, geometry: &ui::Geometry) -> (usize, usize, usize) {
    let area = geometry.context_help.as_ref().unwrap().shell.footer;
    let text: String = (area.x..area.right())
        .map(|x| buffer[(x, area.y)].symbol())
        .collect();
    let mut words = text.split_whitespace();
    assert_eq!(words.next(), Some("Lines"));
    let (first, last) = words.next().unwrap().split_once('–').unwrap();
    assert_eq!(words.next(), Some("of"));
    (
        first.parse().unwrap(),
        last.parse().unwrap(),
        words.next().unwrap().parse().unwrap(),
    )
}

#[test]
fn context_footer_counts_visible_lines_and_end_reaches_the_real_last_line() {
    let mut app = App::new("owned-help-line-range".into());
    app.context_help = Some(ContextHelp {
        topic: HelpTopic::Pattern,
        scroll: 0,
    });
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (buffer, g) = draw(&c.app, ColorLevel::NoColor);
    let (first, last, total) = line_range(&buffer, &g);
    let page = usize::from(g.context_help.as_ref().unwrap().shell.body.height);
    assert_eq!(first, 1);
    assert_eq!(last, page);
    assert!(total > last);
    for (code, expected_first) in [(KeyCode::PageDown, page + 1), (KeyCode::PageUp, 1)] {
        c.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &g,
            Duration::ZERO,
        );
        let (buffer, g) = draw(&c.app, ColorLevel::NoColor);
        assert_eq!(line_range(&buffer, &g).0, expected_first);
    }
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
    let (buffer, g) = draw(&c.app, ColorLevel::NoColor);
    assert_eq!(line_range(&buffer, &g), (total - page + 1, total, total));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
    assert_eq!(c.app.context_help.as_ref().unwrap().scroll, 0);
}

#[test]
fn zero_height_body_reports_no_visible_lines() {
    let mut app = App::new("owned-help-empty-viewport".into());
    app.context_help = Some(ContextHelp {
        topic: HelpTopic::Pattern,
        scroll: 0,
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 7)).unwrap();
    let mut g = None;
    terminal
        .draw(|f| g = Some(ui::render(f, &app, &Theme::new(ColorLevel::NoColor))))
        .unwrap();
    let g = g.unwrap();
    assert_eq!(g.context_help.as_ref().unwrap().shell.body.height, 0);
    let (first, last, total) = line_range(terminal.backend().buffer(), &g);
    assert_eq!((first, last), (0, 0));
    assert!(total > 0);
}
