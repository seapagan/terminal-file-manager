//! Probe the real listing buffer, including deliberately divergent cell evidence.
use super::*;
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use std::sync::atomic::AtomicBool;

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    let mut app = App::new(root.path().into());
    app.dual = true;
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(crate::fs::list(
                root.path(),
                &crate::fs::IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
        app.panes[id].entries[0].ignored = Some(true);
        app.panes[id].select_all();
    }
    (root, app)
}

fn buffer(app: &App, theme: &Theme, width: u16, height: u16) -> (Buffer, Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = super::super::geometry(app, Rect::new(0, 0, width, height));
    terminal
        .draw(|frame| geometry = super::super::render(frame, app, theme))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

#[test]
fn report_distinguishes_nominal_defaults_from_full_width_rendered_cursors() {
    let (_root, mut app) = fixture();
    for (level, border) in [
        (ColorLevel::NoColor, "terminal default / unknown (Reset)"),
        (ColorLevel::Ansi16, "ANSI(Cyan)"),
        (ColorLevel::Ansi256, "indexed(30)"),
        (ColorLevel::TrueColor, "RGB(35,140,150)"),
    ] {
        for active in 0..2 {
            app.active = active;
            let theme = Theme::new(level);
            let (buffer, geometry) = buffer(&app, &theme, 160, 40);
            let lines = report(&buffer, &app, &geometry, &theme);
            println!("{level:?} focus={active}\n{}", lines.join("\n"));
            assert!(
                lines
                    .iter()
                    .any(|line| line.starts_with("body intended: fg=inherit bg=inherit"))
            );
            assert!(
                lines
                    .iter()
                    .any(|line| line.starts_with("unoccupied body rendered")
                        && line.contains("bg=terminal default / unknown (Reset)"))
            );
            assert_rendered_panes(&lines, active, border, &theme);
        }
    }
}

fn assert_rendered_panes(lines: &[String], active: usize, border: &str, theme: &Theme) {
    for id in 0..2 {
        let x = id * 80;
        let actual = lines
            .iter()
            .find(|line| line.starts_with(&format!("border rendered ({x},1)")))
            .unwrap();
        assert!(actual.contains(&format!(
            "fg={}",
            if id as usize == active {
                border
            } else {
                "terminal default / unknown (Reset)"
            }
        )));
        let row = lines
            .iter()
            .find(|line| {
                line.starts_with(&format!(
                    "selected rendered x={}..{} y=2 (78 cells)",
                    x + 1,
                    x + 79
                ))
            })
            .unwrap();
        let style = theme.style(if id as usize == active {
            Role::Cursor
        } else {
            Role::InactiveCursor
        });
        assert!(row.contains(&format!("fg={} bg={}", colour(style.fg), colour(style.bg))));
        assert_eq!(row.contains("BOLD"), id as usize == active);
        assert_eq!(row.contains("REVERSED"), theme.level == ColorLevel::NoColor);
        assert!(!row.contains("DIM"));
    }
}

#[test]
fn report_reads_actual_cell_layers_instead_of_repeating_the_cursor_role() {
    let (_root, app) = fixture();
    let theme = Theme::new(ColorLevel::TrueColor);
    let (mut buffer, geometry) = buffer(&app, &theme, 160, 40);
    buffer[(1, 2)].set_style(
        Style::default()
            .fg(Color::Indexed(123))
            .bg(Color::Rgb(1, 2, 3))
            .underline_color(Color::Red)
            .add_modifier(Modifier::UNDERLINED),
    );
    let lines = report(&buffer, &app, &geometry, &theme);
    let actual = lines
        .iter()
        .find(|line| line.starts_with("selected rendered x=1..2 y=2 (1 cells)"))
        .unwrap();
    assert!(actual.contains("fg=indexed(123) bg=RGB(1,2,3) underline=ANSI(Red)"));
    assert!(actual.contains("UNDERLINED"));
    let nominal = lines
        .iter()
        .find(|line| line.starts_with("selected row intended:"))
        .unwrap();
    assert!(nominal.contains("fg=RGB(245,248,250) bg=RGB(35,85,125)"));
    assert!(!nominal.contains("UNDERLINED"));
}

#[test]
fn tiny_panes_do_not_report_neighbouring_frame_cells_as_their_title() {
    let (_root, app) = fixture();
    let theme = Theme::new(ColorLevel::NoColor);
    let (buffer, geometry) = buffer(&app, &theme, 80, 1);
    let lines = report(&buffer, &app, &geometry, &theme);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("title rendered: unavailable"))
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.starts_with("title rendered ("))
    );
}
