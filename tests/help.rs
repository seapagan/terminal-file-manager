//! Categorised Help remains one document behind a temporary jump picker.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller, chord},
    ui,
};

fn controller() -> Controller {
    Controller::new(
        App::new("owned-help-fixture".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn key(c: &mut Controller, text: &str) {
    let (code, modifiers) = chord(text).unwrap();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(code, modifiers)),
            &g,
            Duration::ZERO
        )
        .is_empty()
    );
}

fn draw(c: &Controller, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &c.app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn continuous_document_has_distinct_sections_and_keeps_all_effective_actions() {
    let mut c = controller();
    key(&mut c, "?");
    let text = draw(&c, 160, 160);
    for section in [
        "Navigation",
        "Selection & Marking",
        "File Operations",
        "Jobs & Progress",
        "Panes & Layout",
        "Views & Metadata",
        "Application & Diagnostics",
    ] {
        assert!(text.contains(section), "missing {section}");
    }
    for entry in &c.app.help_rows {
        assert!(
            text.contains(entry.description),
            "missing {}",
            entry.description
        );
    }
}

#[test]
fn picker_escape_returns_to_original_scroll_then_closes_help() {
    let mut c = controller();
    key(&mut c, "?");
    key(&mut c, "PageDown");
    let before = c.app.overlay_scroll;
    key(&mut c, "/");
    assert!(draw(&c, 120, 32).contains("Jump to section or search commands"));
    key(&mut c, "Down");
    key(&mut c, "Esc");
    assert!(c.app.help);
    assert_eq!(c.app.overlay_scroll, before);
    key(&mut c, "Esc");
    assert!(!c.app.help);
}

#[test]
fn fuzzy_search_jumps_to_a_command_and_never_dispatches_it() {
    let mut c = controller();
    key(&mut c, "?");
    key(&mut c, "/");
    for ch in "crtfl".chars() {
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
            &g,
            Duration::ZERO,
        );
    }
    assert!(draw(&c, 120, 32).contains("Create empty file"));
    key(&mut c, "Enter");
    assert!(c.app.help);
    assert!(draw(&c, 120, 32).contains("Create empty file"));
    assert!(c.app.operations.dialog.is_none() && c.app.operations.outbox.is_empty());
}

#[test]
fn ranking_is_stable_and_searches_effective_keys_sections_and_action_names() {
    use terminal_file_manager::input::{Action, HelpSection, HelpTarget, help_hits};
    let keys = Bindings::new(&std::collections::BTreeMap::from([
        ("F11".into(), Action::CreateFile),
        ("Alt+n".into(), Action::Help),
    ]))
    .unwrap();
    let entries = keys.help_entries();
    let sections = help_hits(&entries, "");
    assert_eq!(sections.len(), 7);
    for (hit, section) in sections.iter().zip(HelpSection::ALL) {
        assert_eq!(hit.target, HelpTarget::Section(section));
    }
    assert_eq!(
        help_hits(&entries, "move")[0].target,
        HelpTarget::Action(Action::Move)
    );
    let hits = help_hits(&entries, "F11");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].target, HelpTarget::Action(Action::CreateFile));
    assert_eq!(hits[0].keys, "F11 / Shift+F7");
    assert!(!help_hits(&entries, "split pane").is_empty());
    assert!(help_hits(&entries, "zzzzzzzz").is_empty());
    for query in ["back", "trash", "move", "pane", "crtfl"] {
        let targets = |hits: Vec<terminal_file_manager::input::HelpHit>| {
            hits.into_iter().map(|hit| hit.target).collect::<Vec<_>>()
        };
        assert_eq!(
            targets(help_hits(&entries, query)),
            targets(help_hits(&entries, query))
        );
    }
}

fn click(c: &mut Controller, area: Rect, width: u16, height: u16) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let g = ui::geometry(&c.app, Rect::new(0, 0, width, height));
    let event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(c.handle(event, &g, Duration::ZERO).is_empty());
}

#[test]
fn header_and_result_hit_regions_jump_to_the_rendered_section_anchor() {
    use terminal_file_manager::input::HelpTarget;
    let mut c = controller();
    key(&mut c, "?");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    click(&mut c, g.help.unwrap().search.unwrap(), 120, 32);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    let layout = g.help.unwrap();
    let picker = layout.picker.unwrap();
    let (area, index) = picker.controls[3];
    let target = picker.hits[index].target;
    let anchor = layout.anchors.iter().find(|(t, _)| *t == target).unwrap().1;
    click(&mut c, area, 120, 32);
    assert!(c.app.help_picker.is_none());
    assert_eq!(c.app.overlay_scroll, anchor.min(layout.scroll_limit));
    let HelpTarget::Section(section) = target else {
        panic!("section result");
    };
    assert!(draw(&c, 120, 32).contains(section.title()));
    assert_eq!(c.app.help_landing.unwrap().0, target);
}

#[test]
fn search_empty_no_result_and_paste_stay_modal_and_leave_panes_untouched() {
    let mut c = controller();
    key(&mut c, "?");
    key(&mut c, "/");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    c.handle(Event::Paste("zzzzzzzz\n\u{1b}".into()), &g, Duration::ZERO);
    assert!(draw(&c, 120, 32).contains("No matching commands"));
    key(&mut c, "Enter");
    assert!(c.app.help_picker.is_some());
    for chord in ["F5", "Alt+b", "Tab", "F12"] {
        key(&mut c, chord);
    }
    assert!(c.app.operations.outbox.is_empty() && c.app.operations.dialog.is_none());
    assert!(c.app.pattern.is_none() && !c.app.diagnostics && c.app.active == 0);
    key(&mut c, "Esc");
    assert!(c.app.help);
}

#[test]
fn every_anchor_counts_wrapped_headings_and_survives_narrow_resize() {
    let mut c = controller();
    key(&mut c, "?");
    for (width, height) in [(1, 1), (12, 5), (20, 12), (40, 16), (80, 24), (120, 32)] {
        let g = ui::geometry(&c.app, Rect::new(0, 0, width, height));
        let layout = g.help.as_ref().unwrap();
        assert_eq!(layout.anchors.len(), c.app.help_rows.len() + 7);
        assert!(layout.anchors.windows(2).all(|pair| pair[0].1 < pair[1].1));
        c.app.overlay_scroll = u16::MAX;
        c.handle(Event::Resize(width, height), &g, Duration::ZERO);
        assert_eq!(c.app.overlay_scroll, layout.scroll_limit);
        draw(&c, width, height);
        c.app.help_picker = Some(Default::default());
        draw(&c, width, height);
        c.app.help_picker = None;
    }
}

#[test]
fn section_and_landing_styles_remain_visible_in_colour_and_monochrome() {
    use ratatui::style::Modifier;
    use terminal_file_manager::input::{HelpSection, HelpTarget};
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let mut c = controller();
        key(&mut c, "?");
        let target = HelpTarget::Section(HelpSection::Navigation);
        c.app.help_landing = Some((target, c.app.now + Duration::from_secs(2)));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
        let layout = g.help.unwrap();
        let theme = Theme::new(level);
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &theme);
            })
            .unwrap();
        let cell = &terminal.backend().buffer()[(layout.body.x, layout.body.y)];
        assert_eq!(cell.symbol(), "N");
        assert!(cell.modifier.contains(Modifier::BOLD));
        if level == ColorLevel::NoColor {
            assert!(cell.modifier.contains(Modifier::REVERSED));
        } else {
            assert_eq!(
                cell.bg,
                theme
                    .style(terminal_file_manager::color::Role::Cursor)
                    .bg
                    .unwrap()
            );
        }
        c.app.now += Duration::from_secs(3);
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &theme);
            })
            .unwrap();
        let cell = &terminal.backend().buffer()[(layout.body.x, layout.body.y)];
        assert!(cell.modifier.contains(Modifier::BOLD | Modifier::DIM));
        assert!(!cell.modifier.contains(Modifier::REVERSED));
        let search = layout.search.unwrap();
        assert_eq!(shown(terminal.backend().buffer(), search), "[/ Jump]");
    }
}

fn shown(buffer: &ratatui::buffer::Buffer, area: Rect) -> String {
    (area.x..area.right())
        .map(|x| buffer[(x, area.y)].symbol())
        .collect()
}

#[test]
fn picker_ignores_alt_quit_and_release_events_before_independent_escape_presses() {
    use crossterm::event::KeyEventKind;
    let mut c = controller();
    key(&mut c, "?");
    key(&mut c, "/");
    key(&mut c, "Alt+q");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    for code in [KeyCode::Char('q'), KeyCode::Esc, KeyCode::Enter] {
        let key = KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Release);
        c.handle(Event::Key(key), &g, Duration::ZERO);
    }
    assert!(!c.quit);
    assert!(c.app.help);
    assert!(c.app.help_picker.as_ref().unwrap().query.is_empty());
    key(&mut c, "Esc");
    assert!(c.app.help_picker.is_none());
    assert!(c.app.help);
    key(&mut c, "Esc");
    assert!(!c.app.help);
    assert!(!c.quit);
    key(&mut c, "q");
    assert!(c.quit);
}
