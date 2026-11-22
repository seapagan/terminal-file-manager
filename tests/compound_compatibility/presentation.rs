use super::*;
use terminal_file_manager::settings::compound_check::difference;

#[test]
fn desktop_popup_shows_complete_lists_and_exact_set_differences() {
    let c = controller(
        "compound_extensions = ['.tar.gz', '.custom.js']\ncompound_extensions_extend = ['.tar.gz', '.tar.xz', '.user.js']",
        "0.1.0",
    );
    let review = c.app.compound_extension_review.as_ref().unwrap();
    let (text, g) = draw(&c, 120, 50);
    for label in [
        "Your configured extensions:",
        "Current TFM defaults:",
        "Differences:",
        "Only in your configuration:",
        "Only in current defaults:",
        "Additive entries now in TFM defaults:",
        "Safe to remove with your current choice:",
        "Keep my configuration",
        "Use current defaults",
        "Remove redundant entries",
    ] {
        assert!(text.contains(label), "{label}\n{text}");
    }
    for suffix in &review.defaults {
        assert!(text.contains(&format!("{suffix:?}")), "{suffix}\n{text}");
    }
    assert!(text.contains("[\".tar.gz\", \".custom.js\"]"));
    assert!(text.contains("Only in your configuration: [\".custom.js\"]"));
    assert!(text.contains("Safe to remove with your current choice: [\".tar.gz\"]"));
    assert!(text.contains("Retain to preserve your chosen replacement: [\".tar.xz\"]"));
    assert!(!text.contains("Safe to remove with your current choice: [\".tar.gz\", \".tar.xz\"]"));
    assert_eq!(
        difference(review.configured.as_ref().unwrap(), &review.defaults),
        [".custom.js"]
    );
    assert_eq!(
        difference(&review.defaults, review.configured.as_ref().unwrap()),
        [
            ".tar.xz",
            ".tar.bz2",
            ".tar.zst",
            ".tar.lz",
            ".tar.lzma",
            ".tar.Z"
        ]
    );
    assert_eq!(g.compound_extension_review.unwrap().limit, 0);
}

#[test]
fn narrow_popup_scrolls_to_all_configured_values_and_updates_safe_cleanup_evidence() {
    let settings = Settings {
        compound_extensions: Some((0..35).map(|n| format!(".custom.{n:02}")).collect()),
        compound_extensions_extend: vec![".tar.gz".into()],
        ..Default::default()
    };
    let mut c = controller(&toml::to_string(&settings).unwrap(), "0.1.0");
    let mut seen = String::new();
    loop {
        let (text, g) = draw(&c, 54, 20);
        let layout = g.compound_extension_review.unwrap();
        seen.push_str(&visible_body(&text, layout.shell.body));
        if layout.scroll == layout.limit {
            break;
        }
        event(
            &mut c,
            Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            54,
            20,
        );
    }
    for value in settings.compound_extensions.unwrap() {
        assert!(seen.contains(&value), "{value}");
    }
    event(&mut c, Event::Resize(120, 50), 54, 20);
    key(&mut c, KeyCode::Char('d'));
    key(&mut c, KeyCode::End);
    let (text, _) = draw(&c, 120, 50);
    assert!(text.contains("Safe to remove with your current choice: [\".tar.gz\"]"));
    assert!(text.contains("Retain to preserve your chosen replacement: []"));
}

#[test]
fn tiny_terminals_cannot_activate_hidden_choices_but_can_dismiss_safely() {
    for (width, height) in [(0, 0), (1, 1), (10, 3), (20, 5)] {
        let mut c = controller("compound_extensions = []", "0.1.0");
        draw(&c, width, height);
        for code in [KeyCode::Char('d'), KeyCode::Enter] {
            event(
                &mut c,
                Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                width,
                height,
            );
        }
        assert!(c.app.compound_extension_review.is_some());
        assert_eq!(c.preferences.compound_extensions, Some(vec![]));
        event(
            &mut c,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            width,
            height,
        );
        assert!(c.app.compound_extension_review.is_none());
        assert_eq!(c.preferences.compound_extensions, Some(vec![]));
    }
}

fn visible_body(text: &str, body: Rect) -> String {
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
