use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};
use terminal_file_manager::{
    app::App,
    color::Theme,
    operations::{Dialog, Operation, Plan},
    ui,
};
mod dialog_fixture;
use dialog_fixture::dialog;
use terminal_file_manager::color::Role;

fn rendered(app: &App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect()
}

#[cfg(unix)]
#[test]
fn unedited_native_destination_remains_visible_without_a_duplicate_path() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let target = root
        .path()
        .join(OsString::from_vec(b"destination\xff".to_vec()));
    let file = root.path().join("file");
    std::fs::write(&file, b"data").unwrap();
    let mut app = dialog(Plan::paths(Operation::Copy, vec![file], Some(target.clone())).unwrap());
    if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
        d.native_target = target.clone();
    }
    let text = rendered(&app, 120, 40).join("\n");
    assert!(text.contains(&ui::clean(&format!("{target:?}"))));
    assert!(!text.contains("Approved destination:"));
    assert!(
        matches!(&app.operations.dialog,Some(Dialog::Operation(d)) if d.text.is_empty() && !d.edited && d.native_target == target)
    );
}

#[test]
fn populated_form_values_keep_normal_styles_and_a_separate_caret() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"data").unwrap();
    for operation in [
        Operation::Copy,
        Operation::Move,
        Operation::Rename,
        Operation::Permissions(0),
    ] {
        let mut app =
            dialog(Plan::paths(operation, vec![path.clone()], Some(root.path().into())).unwrap());
        if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
            d.text = "typed-value".into();
        }
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::TrueColor,
        ] {
            let theme = Theme::new(level);
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|frame| {
                    ui::render(frame, &app, &theme);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let (x, y) = (0..40)
                .find_map(|y| {
                    let row: String = (0..120).map(|x| buffer[(x, y)].symbol()).collect();
                    row.find("typed-value▏")
                        .map(|start| (ratatui::text::Line::raw(&row[..start]).width() as u16, y))
                })
                .unwrap();
            assert_eq!(
                buffer[(x, y)].fg,
                theme.style(Role::Value).fg.unwrap_or_default()
            );
            assert!(
                !buffer[(x, y)]
                    .modifier
                    .contains(ratatui::style::Modifier::REVERSED)
            );
            assert_eq!(buffer[(x + 11, y)].symbol(), "▏");
        }
    }
}

#[test]
fn floating_metadata_uses_the_file_row_without_changing_the_sidebar() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("child")).unwrap();
    let snapshot = terminal_file_manager::fs::list(
        root.path(),
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Ok(snapshot));
    app.request_metadata(0);
    app.popup = true;
    app.panes[0].entries.clear();
    app.panes[0].visible.clear();
    let text = rendered(&app, 120, 40).join("\n");
    assert!(text.contains("d  child/"));
    let row = text
        .lines()
        .find(|line| line.contains("d  child/"))
        .unwrap();
    assert!(row.contains("—"));
}

#[test]
fn cached_file_types_and_whole_sizes_share_columns_without_changing_paths() {
    use terminal_file_manager::fs::EntryKind::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("seed");
    std::fs::write(&path, b"seed").unwrap();
    let mut plan = Plan::paths(Operation::Trash, vec![path], None).unwrap();
    let seed = plan.operands[0].clone();
    let cases = [
        (Directory, "d", true, false, false),
        (Junction, "j", true, false, false),
        (Link, "@", true, false, false),
        (Link, "!", false, false, true),
        (File, "*", false, true, false),
        (Device, "D", false, false, false),
        (ReparsePoint, "r", false, false, false),
        (Socket, "s", false, false, false),
        (Fifo, "p", false, false, false),
        (Unknown, "?", false, false, false),
    ];
    plan.operands = cases
        .iter()
        .enumerate()
        .map(|(i, (kind, _, enterable, executable, error))| {
            let mut entry = seed.clone();
            entry.kind = *kind;
            entry.enterable = *enterable;
            entry.executable = *executable;
            entry.error = error.then(|| "broken link".into());
            entry.path = root
                .path()
                .join(format!("{i}資料e\u{301}{}", "long".repeat(30)));
            entry.size = Some(10240);
            entry
        })
        .collect();
    let native = plan.operands.clone();
    let app = dialog(plan);
    assert_type_columns(&app, &cases);
    assert!(
        matches!(&app.operations.dialog, Some(Dialog::Operation(d)) if d.plan.operands == native)
    );
}

fn assert_type_columns(
    app: &App,
    cases: &[(terminal_file_manager::fs::EntryKind, &str, bool, bool, bool)],
) {
    for width in [20, 40, 80, 120] {
        let rows = rendered(app, width, 80);
        let mut endings = Vec::new();
        for &(_, indicator, _, _, _) in cases {
            let row = rows
                .iter()
                .find(|row| {
                    row.contains(&format!("{indicator}  "))
                        && (row.contains("—") || row.contains("KiB"))
                })
                .unwrap();
            if indicator == "d" || indicator == "j" {
                assert!(row.contains('/'), "{row}");
            }
            if indicator == "@" {
                assert!(!row.contains('/'), "{row}");
            }
            let ending = if indicator == "*" { "10 KiB" } else { "—" };
            let start = row.rfind(ending).unwrap();
            endings.push(row[start + ending.len()..].chars().count());
        }
        assert!(endings.iter().all(|end| *end == endings[0]));
    }
}

#[test]
fn recursive_preview_summary_is_complete_but_rows_only_show_selected_roots() {
    use terminal_file_manager::operations::{Queue, Update};
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("selected");
    std::fs::create_dir_all(selected.join("nested")).unwrap();
    std::fs::write(selected.join("nested/hidden-descendant"), vec![0; 1024]).unwrap();
    let plan = Plan::paths(Operation::Delete, vec![selected], None).unwrap();
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    let plan = loop {
        if let Update::Preview { plan, .. } = queue.next(std::time::Duration::from_secs(5)).unwrap()
        {
            break *plan;
        }
    };
    let app = dialog(plan);
    let text = rendered(&app, 120, 40).join("\n");
    assert!(text.contains("1 file, 2 directories · 1 KiB"));
    assert!(text.contains("d  selected/"));
    assert!(!text.contains("hidden-descendant"));
    assert!(!text.contains("nested"));
    assert!(!text.contains("Local operation:"));
    assert!(!text.contains("Preview:"));
}

#[test]
fn transfer_fields_stay_fixed_and_verification_has_no_duplicate_destination() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"data").unwrap();
    for operation in [Operation::Copy, Operation::Move] {
        let mut app =
            dialog(Plan::paths(operation, vec![path.clone()], Some(root.path().into())).unwrap());
        for checksum in [false, true] {
            if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
                d.options.checksum = checksum;
            }
            let text = rendered(&app, 120, 40).join("\n");
            assert!(text.contains(if checksum {
                "Verification: SHA-256 + metadata"
            } else {
                "Verification: Metadata"
            }));
            assert!(!text.contains("Approved destination"));
            assert!(!text.contains("F2 toggles"));
            assert!(!text.contains("not byte equality"));
            assert!(text.contains("1 file selected"));
        }
    }
}

#[test]
fn mkdir_orders_context_before_a_precisely_styled_field() {
    let app = dialog(Plan::mkdir("/controlled".into()));
    let theme = Theme::new(ColorLevel::Ansi256);
    let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows: Vec<String> = (0..30)
        .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    let parent = rows.iter().position(|row| row.contains("Parent:")).unwrap();
    let field = rows
        .iter()
        .position(|row| row.contains("Directory name:"))
        .unwrap();
    assert!(parent < field);
    assert!(!rows[field].contains('>'));
    assert!(rows[field].contains('▏'));
    assert!(rows.iter().any(|row| row.contains("[Enter Create]")));
    let x = rows[field].find("Directory name:").unwrap() as u16;
    assert_eq!(
        buffer[(x, field as u16)].fg,
        theme.style(Role::Label).fg.unwrap()
    );
    assert!(
        !buffer[(x, field as u16)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED)
    );
}

#[test]
fn special_selection_objects_use_an_honest_neutral_count() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("seed");
    std::fs::write(&path, "owned").unwrap();
    for operation in [Operation::Copy, Operation::Move, Operation::Rename] {
        let mut plan =
            Plan::paths(operation, vec![path.clone()], Some(root.path().into())).unwrap();
        for kind in [
            terminal_file_manager::fs::EntryKind::Link,
            terminal_file_manager::fs::EntryKind::Junction,
            terminal_file_manager::fs::EntryKind::Socket,
        ] {
            plan.operands[0].kind = kind;
            let text = rendered(&dialog(plan.clone()), 120, 40).join("\n");
            assert!(text.contains("1 entry selected"), "{text}");
            assert!(!text.contains("1 file selected"));
        }
    }
}

#[test]
fn mixed_special_file_and_directory_selections_are_entries() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    let directory = root.path().join("directory");
    std::fs::write(&file, b"owned").unwrap();
    std::fs::create_dir(&directory).unwrap();
    for operation in [Operation::Copy, Operation::Move] {
        let mut plan = Plan::paths(
            operation,
            vec![file.clone(), directory.clone()],
            Some(root.path().into()),
        )
        .unwrap();
        plan.operands.push(plan.operands[0].clone());
        plan.operands[2].kind = terminal_file_manager::fs::EntryKind::Link;
        let text = rendered(&dialog(plan), 120, 40).join("\n");
        assert!(text.contains("3 entries selected"));
        assert!(!text.contains("2 files"));
    }
}
