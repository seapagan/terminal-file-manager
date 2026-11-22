use super::*;
use terminal_file_manager::{settings::compound_check::Stage, ui::CheckIntent};

#[test]
fn replacement_dialog_groups_complete_actual_values_without_debug_or_empty_sections() {
    let c = controller(
        "compound_extensions = ['.tar.gz', '.tar.xz', '.tar.Z']",
        "0.1.0",
    );
    let (text, g) = draw(&c, 120, 40);
    let layout = g.compound_extension_review.unwrap();
    let body = body_rows(&text, layout.shell.body);
    assert_eq!(
        body,
        [
            "Your configured extensions (3)",
            "  .tar.gz   .tar.xz   .tar.Z",
            "",
            "Current TFM defaults (7)",
            "  .tar.gz   .tar.xz   .tar.bz2   .tar.zst   .tar.lz",
            "  .tar.lzma   .tar.Z",
            "",
            "Additional extensions in TFM's defaults",
            "  + .tar.bz2   + .tar.zst   + .tar.lz   + .tar.lzma",
            "",
            "Which list would you like to use?",
        ]
    );
    assert_eq!(layout.limit, 0);
    assert!(layout.shell.outer.width <= 60 && layout.shell.outer.height <= 20);
    assert!(!text.contains("Extensions removed by using defaults"));
    assert!(!text.contains("Review custom extensions"));
    assert_plain(&text);
}

#[test]
fn additive_dialog_shows_only_safe_duplicates_and_identifies_custom_entries_to_keep() {
    let c = controller(
        "compound_extensions_extend = ['.tar.gz', '.user.js']",
        "0.1.0",
    );
    let (text, g) = draw(&c, 80, 24);
    let layout = g.compound_extension_review.unwrap();
    assert_eq!(layout.stage, Stage::Additions);
    for value in [
        "Review custom extensions",
        "Your added extensions (2)",
        ".tar.gz",
        ".user.js",
        "Already included",
        "✓ .tar.gz",
        "Custom additions to keep",
        "Keep additions",
        "Remove duplicates",
    ] {
        assert!(text.contains(value), "{value}\n{text}");
    }
    assert!(!text.contains("✓ .user.js"));
    assert!(!text.contains("Current TFM defaults"));
    assert_eq!(layout.shell.actions.len(), 2);
    assert!(layout.shell.actions.iter().all(|a| a.action.enabled));
    assert_eq!(layout.limit, 0);
    assert!(layout.shell.outer.width <= 60 && layout.shell.outer.height <= 18);
    assert_plain(&text);
}

#[test]
fn removal_and_addition_differences_use_semantic_colors_and_monochrome_markers() {
    use terminal_file_manager::color::Role;
    for level in [
        colored_text::ColorLevel::NoColor,
        colored_text::ColorLevel::TrueColor,
    ] {
        let c = controller("compound_extensions = ['.tar.gz', '.custom.js']", "0.1.0");
        let (t, g) = canvas(&c, 120, 40, level);
        let buffer = t.backend().buffer();
        let theme = Theme::new(level);
        for (marker, role) in [("+ .tar.xz", Role::Marked), ("- .custom.js", Role::Warning)] {
            let point = find(buffer, marker);
            assert_eq!(
                buffer[point].fg,
                theme.style(role).fg.unwrap_or(ratatui::style::Color::Reset)
            );
        }
        let actions = &g.compound_extension_review.unwrap().shell.actions;
        assert_eq!(actions[0].action.intent, CheckIntent::Keep);
        assert_ne!(
            buffer[(actions[0].area.x, actions[0].area.y)].style(),
            buffer[(actions[1].area.x, actions[1].area.y)].style()
        );
    }
}

#[test]
fn empty_replacement_is_explicit_and_unnecessary_additive_explanations_are_absent() {
    let mut c = controller(
        "compound_extensions = []\ncompound_extensions_extend = ['.tar.gz']",
        "0.1.0",
    );
    let (text, _) = draw(&c, 120, 40);
    assert!(text.contains("Your configured extensions (0)"));
    assert!(text.contains("No compound extensions"));
    assert!(!text.contains("Extensions removed by using defaults"));
    key(&mut c, KeyCode::Enter);
    assert!(c.app.compound_extension_review.is_none());
    let c = controller("compound_extensions_extend = ['.tar.gz']", "0.1.0");
    let (text, _) = draw(&c, 120, 40);
    assert!(!text.contains("Custom additions to keep"));
    assert!(!text.contains("No compound extensions"));
}

#[test]
fn responsive_action_groups_remain_centered_clickable_and_separate_from_the_body() {
    for settings in [
        "compound_extensions = ['.custom.js']",
        "compound_extensions_extend = ['.tar.gz', '.user.js']",
    ] {
        let c = controller(settings, "0.1.0");
        for width in [28, 32, 40, 60, 80, 120] {
            let (_, g) = draw(&c, width, 30);
            let layout = g.compound_extension_review.unwrap();
            assert_eq!(layout.shell.actions.len(), 2, "width {width}");
            for region in &layout.shell.actions {
                assert!(region.area.y >= layout.shell.body.bottom());
                assert_eq!(
                    layout.shell.hit(region.area.x, region.area.y),
                    Some(&region.action.intent)
                );
                let peers: Vec<_> = layout
                    .shell
                    .actions
                    .iter()
                    .filter(|a| a.area.y == region.area.y)
                    .collect();
                let left = peers.first().unwrap().area.x - layout.shell.body.x;
                let right = layout.shell.body.right() - peers.last().unwrap().area.right();
                assert!(left.abs_diff(right) <= 1);
            }
        }
    }
}

#[test]
fn both_dialogs_scroll_long_unicode_values_without_elision_and_clamp_after_resize() {
    for additions in [false, true] {
        let values: Vec<_> = (0..24)
            .map(|n| format!(".custom.{n:02}"))
            .chain([format!(".{}", "long".repeat(24)), ".資料e\u{301}.js".into()])
            .collect();
        let settings = if additions {
            Settings {
                compound_extensions: Some(values.clone()),
                compound_extensions_extend: values.clone(),
                ..Default::default()
            }
        } else {
            Settings {
                compound_extensions: Some(values.clone()),
                ..Default::default()
            }
        };
        let mut c = controller(&toml::to_string(&settings).unwrap(), "0.1.0");
        if additions {
            key(&mut c, KeyCode::Enter);
        }
        let seen = scroll_all(&mut c, 40, 18);
        for value in values {
            assert!(seen.contains(&value), "{value}\n{seen}");
        }
        event(&mut c, Event::Resize(120, 60), 40, 18);
        let (_, g) = draw(&c, 120, 60);
        let layout = g.compound_extension_review.unwrap();
        assert_eq!(
            c.app.compound_extension_review.as_ref().unwrap().scroll,
            layout.scroll
        );
        assert!(layout.scroll <= layout.limit);
        key(&mut c, KeyCode::Home);
        assert_eq!(c.app.compound_extension_review.as_ref().unwrap().scroll, 0);
    }
}

#[test]
fn tiny_terminals_block_hidden_changes_and_escape_keeps_each_configuration() {
    for text in [
        "compound_extensions = []",
        "compound_extensions_extend = ['.tar.gz']",
    ] {
        for (width, height) in [(0, 0), (1, 1), (10, 3), (20, 5)] {
            let mut c = controller(text, "0.1.0");
            draw(&c, width, height);
            for code in [KeyCode::Char('d'), KeyCode::Char('r')] {
                event(
                    &mut c,
                    Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                    width,
                    height,
                );
            }
            assert!(c.app.compound_extension_review.is_some());
            event(
                &mut c,
                Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                width,
                height,
            );
            assert!(c.app.compound_extension_review.is_none());
            let original = Settings::parse(text).unwrap();
            assert_eq!(
                c.preferences.compound_extensions,
                original.compound_extensions
            );
            assert_eq!(
                c.preferences.compound_extensions_extend,
                original.compound_extensions_extend
            );
        }
    }
}

fn scroll_all(c: &mut Controller, width: u16, height: u16) -> String {
    let mut seen = String::new();
    loop {
        let (canvas, g) = canvas(c, width, height, colored_text::ColorLevel::NoColor);
        let layout = g.compound_extension_review.unwrap();
        seen.push_str(&body_values(canvas.backend().buffer(), layout.shell.body));
        if layout.scroll == layout.limit {
            break;
        }
        let before = layout.scroll;
        event(
            c,
            Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            width,
            height,
        );
        assert!(c.app.compound_extension_review.as_ref().unwrap().scroll > before);
    }
    seen
}

fn body_rows(text: &str, body: Rect) -> Vec<String> {
    text.lines()
        .skip(usize::from(body.y))
        .take(usize::from(body.height))
        .map(|line| {
            let row: String = line
                .chars()
                .skip(usize::from(body.x))
                .take(usize::from(body.width))
                .collect();
            row.trim_end().to_owned()
        })
        .collect()
}

fn assert_plain(text: &str) {
    for excluded in ["[\"", "Some(", "Continue", ": On", ": Off", "[]"] {
        assert!(!text.contains(excluded), "{excluded}\n{text}");
    }
}

fn find(buffer: &ratatui::buffer::Buffer, needle: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if let Some(index) = row.find(needle) {
            return (row[..index].chars().count() as u16, y);
        }
    }
    panic!("missing {needle}");
}

fn body_values(buffer: &ratatui::buffer::Buffer, body: Rect) -> String {
    let mut text = String::new();
    for y in body.y..body.bottom() {
        let mut row = String::new();
        let mut x = body.x;
        while x < body.right() {
            let symbol = buffer[(x, y)].symbol();
            row.push_str(symbol);
            x = x.saturating_add(ui::modal::width(symbol).max(1));
        }
        let row = row.trim_end();
        text.push_str(row.strip_prefix("  ").unwrap_or(row));
    }
    text
}
