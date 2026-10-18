//! Native Windows metadata/reparse contracts; Linux xwin supplies compile evidence only.
#![cfg(windows)]
use std::{fs, process::Command, sync::atomic::AtomicBool};
use terminal_file_manager::{
    Invocation,
    fs::{self as local, EntryKind, IgnoreContext},
};

#[test]
fn windows_hidden_attribute_is_independent_of_dot_names() {
    let root = tempfile::tempdir().unwrap();
    let dotted = root.path().join(".dot-name");
    fs::write(&dotted, "data").unwrap();
    assert!(!local::inspect(&dotted).hidden);
    let hidden = root.path().join("hidden-name");
    fs::write(&hidden, "data").unwrap();
    let output = Command::new("attrib")
        .arg("+h")
        .arg(&hidden)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let entry = local::inspect(&hidden);
    assert!(entry.hidden);
    assert_eq!(entry.kind, EntryKind::File);
    assert_eq!(fs::read(&hidden).unwrap(), b"data");
}

#[test]
fn junctions_are_native_nonrecursive_and_preserve_trailing_paths() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let junction = root.path().join("junction");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("payload"), b"1234").unwrap();
    let output = Command::new("cmd")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let snapshot = local::list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_junction_rendering(&snapshot);
    let link = snapshot
        .entries
        .iter()
        .find(|e| e.path == junction)
        .unwrap();
    assert_eq!(link.kind, EntryKind::Junction);
    assert!(link.enterable);
    assert!(!link.may_recurse());
    assert!(link.link_target.is_some());
    let mut trailing = junction.as_os_str().to_owned();
    trailing.push("\\");
    let trailing_entry = local::inspect(std::path::Path::new(&trailing));
    assert_eq!(trailing_entry.kind, EntryKind::Junction);
    assert_eq!(
        trailing_entry.identity.unwrap().object,
        link.identity.as_ref().unwrap().object
    );
    assert!(local::totals(&junction, &AtomicBool::new(false)).is_err());
    assert_totals(root.path(), (1, 1, 4), None);
    fs::remove_file(target.join("payload")).unwrap();
    fs::remove_dir(&target).unwrap();
    let broken = local::inspect(&junction);
    assert_eq!(broken.kind, EntryKind::Junction);
    assert!(broken.error.is_some());
    assert_totals(root.path(), (0, 0, 0), None);
    assert_broken_junction_navigation_and_grouping(root.path(), &junction);
    fs::remove_dir(&junction).unwrap();
}

#[test]
fn symlinks_are_classified_without_counting_their_targets() {
    let root = tempfile::tempdir().unwrap();
    let scan = root.path().join("scan");
    let target = root.path().join("target");
    fs::create_dir(&scan).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(scan.join("ordinary"), b"123").unwrap();
    fs::write(target.join("excluded"), b"12345").unwrap();
    let link = scan.join("link");
    std::os::windows::fs::symlink_dir(&target, &link).unwrap();
    let entry = local::inspect(&link);
    assert_eq!(entry.kind, EntryKind::Link);
    assert!(entry.enterable && !entry.may_recurse());
    assert!(entry.link_target.is_some());
    assert_totals(&scan, (1, 0, 3), None);
    assert_eq!(fs::read(target.join("excluded")).unwrap(), b"12345");
    fs::remove_file(target.join("excluded")).unwrap();
    fs::remove_dir(&target).unwrap();
    assert!(local::inspect(&link).error.is_some());
    assert_totals(&scan, (1, 0, 3), None);
}

fn assert_junction_rendering(snapshot: &local::Snapshot) {
    use colored_text::ColorLevel;
    use ratatui::{Terminal, backend::TestBackend};
    use terminal_file_manager::{app::App, color::Theme, ui, view::Columns};
    let mut app = App::new(snapshot.path.clone());
    app.complete_listing(0, 1, Ok(snapshot.clone()));
    app.panes[0].columns = Columns {
        size: false,
        modified: false,
        attributes: false,
        owner: false,
    };
    app.panes[0].cursor = None;
    let row = app.panes[0]
        .visible
        .iter()
        .position(|i| app.panes[0].entries[*i].name == "junction")
        .unwrap() as u16
        + 2;
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, row)].symbol(), "j");
    let name = (8..79)
        .map(|x| buffer[(x, row)].symbol())
        .collect::<String>();
    assert_eq!(name.trim_end(), "junction");
}

#[test]
fn unc_drive_and_extended_paths_survive_cli_parsing_without_network_access() {
    for path in [
        r"\\server\share\directory",
        r"C:\directory with spaces",
        r"\\?\C:\long\directory",
    ] {
        let result = terminal_file_manager::run(&[path.into()]).unwrap();
        match result {
            Invocation::Browse(native) => assert_eq!(native, std::path::PathBuf::from(path)),
            _ => panic!("path was not preserved"),
        }
    }
}

fn assert_totals(
    path: &std::path::Path,
    expected: (u64, u64, u64),
    error_path: Option<&std::path::Path>,
) {
    let totals = local::totals(path, &AtomicBool::new(false)).unwrap();
    assert_eq!((totals.files, totals.directories, totals.bytes), expected);
    if let Some(path) = error_path {
        assert_eq!(totals.errors.len(), 1);
        assert!(totals.errors[0].contains(&path.display().to_string()));
    } else {
        assert!(totals.errors.is_empty());
    }
}

fn assert_broken_junction_navigation_and_grouping(
    root: &std::path::Path,
    junction: &std::path::Path,
) {
    use terminal_file_manager::{
        app::App,
        input::{Action, Bindings, Controller},
        ui,
    };
    fs::create_dir(root.join("zz-directory")).unwrap();
    fs::write(root.join("aaa-file"), "data").unwrap();
    let broken = local::inspect(junction);
    assert_eq!(broken.kind, EntryKind::Junction);
    assert!(!broken.enterable && !broken.may_recurse());
    assert!(broken.error.is_some() && broken.link_target.is_some());
    let snapshot = local::list(root, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap();
    let mut app = App::new(root.into());
    assert!(app.complete_listing(0, 1, Ok(snapshot)));
    let pane = &mut app.panes[0];
    let names: Vec<_> = pane
        .entries
        .iter()
        .map(|e| e.name.to_str().unwrap())
        .collect();
    assert_eq!(names, ["zz-directory", "aaa-file", "junction"]);
    pane.cursor = Some(junction.into());
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    assert!(c.dispatch(Action::Enter, &g).is_empty());
    assert!(c.app.panes[0].pending.is_none());
    assert_eq!(c.app.panes[0].path, root);
}
