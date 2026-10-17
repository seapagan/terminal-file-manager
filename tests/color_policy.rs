//! Exercise the actual colour policy in isolated child environments.
use colored_text::{ColorizeConfig, RenderTarget};
use std::process::Command;
use terminal_file_manager::color::{Role, Theme};

#[test]
fn environment_policy_preserves_no_color_and_force_precedence() {
    if let Ok(expected) = std::env::var("TFM_EXPECT_LEVEL") {
        assert_detected_policy(&expected);
        return;
    }
    for (values, expected) in [
        (vec![("NO_COLOR", "1"), ("FORCE_COLOR", "3")], "NoColor"),
        (vec![("NO_COLOR", ""), ("CLICOLOR_FORCE", "1")], "NoColor"),
        (vec![("FORCE_COLOR", "0")], "NoColor"),
        (vec![("FORCE_COLOR", "1")], "Ansi16"),
        (vec![("FORCE_COLOR", "2")], "Ansi256"),
        (vec![("FORCE_COLOR", "3"), ("CLICOLOR", "0")], "TrueColor"),
        (vec![("CLICOLOR", "0")], "NoColor"),
        (vec![("TERM", "dumb")], "NoColor"),
        (
            vec![("TERM", "xterm-256color"), ("COLORTERM", "truecolor")],
            "NoColor",
        ),
    ] {
        // Relaunches this test for colour precedence without a security decision.
        // nosemgrep: rust.lang.security.current-exe.current-exe
        let mut child = Command::new(std::env::current_exe().unwrap());
        for key in [
            "NO_COLOR",
            "FORCE_COLOR",
            "CLICOLOR_FORCE",
            "CLICOLOR",
            "TERM",
        ] {
            child.env_remove(key);
        }
        let result = child
            .args([
                "--exact",
                "environment_policy_preserves_no_color_and_force_precedence",
            ])
            .envs(values)
            .env("TFM_EXPECT_LEVEL", expected)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}

fn assert_detected_policy(expected: &str) {
    let level = ColorizeConfig::color_level(RenderTarget::Stdout);
    assert_eq!(format!("{level:?}"), expected);
    let terminal = ColorizeConfig::color_level(RenderTarget::Terminal(true));
    // The xterm/truecolor fixture is the positive terminal control.
    let terminal_expected = if std::env::var("TERM").as_deref() == Ok("xterm-256color") {
        "TrueColor"
    } else {
        expected
    };
    assert_eq!(format!("{terminal:?}"), terminal_expected);
    let style = Theme::new(level).style(Role::Directory);
    assert_eq!(style.fg.is_none(), expected == "NoColor");
    assert_rendered_policy(level);
}

fn assert_rendered_policy(level: colored_text::ColorLevel) {
    use ratatui::{Terminal, backend::TestBackend, style::Color};
    use terminal_file_manager::{
        app::App,
        fs::{self, IgnoreContext},
        ui,
    };
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("policy-entry"), "data").unwrap();
    let mut app = App::new(root.path().into());
    app.dual = true;
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(fs::list(
                root.path(),
                &IgnoreContext::isolated(),
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap()),
        );
    }
    let mut terminal = Terminal::new(TestBackend::new(100, 16)).unwrap();
    let theme = Theme::for_stdout();
    assert_eq!(theme.level, level);
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    assert_selection_rows(terminal.backend().buffer(), &theme);
    if level == colored_text::ColorLevel::NoColor {
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset)
        );
    }
}

fn assert_selection_rows(buffer: &ratatui::buffer::Buffer, theme: &Theme) {
    use ratatui::style::Modifier;
    let level = theme.level;
    for (id, range) in [(0, 1..49), (1, 51..99)] {
        let role = if id == 0 {
            Role::Cursor
        } else {
            Role::InactiveCursor
        };
        let style = theme.style(role);
        for x in range {
            let cell = &buffer[(x, 2)];
            assert_eq!((cell.fg, cell.bg), (style.fg.unwrap(), style.bg.unwrap()));
            assert_eq!(
                cell.modifier.contains(Modifier::REVERSED),
                level == colored_text::ColorLevel::NoColor
            );
        }
    }
}
