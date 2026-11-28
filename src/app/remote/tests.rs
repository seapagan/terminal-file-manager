use super::*;
use crate::{
    color::Theme,
    fs::{self, IgnoreContext},
    input::{Action, Bindings, Controller},
    listing::Sort,
    settings::Settings,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Cell, layout::Rect};
use russh_sftp::protocol::FileAttributes;

const REMOTE_PATH: &[u8] = b"/remote-\xff";

fn entry(name: &[u8], attrs: FileAttributes, order: usize) -> Entry {
    let mut path = REMOTE_PATH.to_vec();
    path.push(b'/');
    path.extend_from_slice(name);
    Entry::from_sftp_metadata(path.into(), name.into(), attrs, order)
}

fn local_app(root: &std::path::Path) -> App {
    let mut app = App::new(root.into());
    let snapshot = fs::list(root, &IgnoreContext::isolated(), &false.into()).unwrap();
    assert!(app.complete_listing(0, 1, Ok(snapshot)));
    app.panes[0].move_cursor(1, 20);
    app
}

fn row(app: &App) -> Vec<Cell> {
    let mut terminal = Terminal::new(TestBackend::new(108, 14)).unwrap();
    let theme = Theme::new(colored_text::ColorLevel::Ansi256);
    let mut pane = Rect::default();
    terminal
        .draw(|frame| {
            pane = ui::render(frame, app, &theme).panes[app.active].unwrap();
        })
        .unwrap();
    let y = pane.y + 2 + app.panes[app.active].row_index().unwrap() as u16;
    let buffer = terminal.backend().buffer();
    (pane.x + 1..pane.right() - 1)
        .map(|x| buffer[(x, y)].clone())
        .collect()
}

#[test]
fn remote_and_local_rows_render_identical_cells_for_equivalent_metadata() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"fixture").unwrap();
    let mut app = local_app(root.path());
    let remote = entry(
        b"file",
        FileAttributes {
            size: Some(2048),
            uid: Some(42),
            gid: Some(43),
            permissions: Some(0o100750),
            mtime: Some(1_700_000_000),
            ..Default::default()
        },
        0,
    );
    let local = &mut app.panes[0].entries[0];
    // Supply equivalent cached POSIX metadata on all hosts, including Windows.
    local.mode = Some(0o100750);
    local.modified = Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000));
    local.size = Some(2048);
    local.executable = true;
    local.windows_attributes = None;
    local.owner = Some(crate::owner::Ownership {
        uid: 42,
        gid: 43,
        user: "42".into(),
        group: "43".into(),
    });
    app.panes[0].refilter();
    app.install_remote_rows(REMOTE_PATH.into(), vec![remote]);
    app.dual = false;
    let local_row = row(&app);
    app.active = 1;
    assert_eq!(row(&app), local_row);
    app.panes[1].columns.owner = false;
    assert_ne!(row(&app), local_row);
}

#[test]
fn missing_remote_metadata_uses_the_shared_unavailable_cells() {
    let mut app = App::new("/local".into());
    let remote = entry(b"unknown", FileAttributes::default(), 0);
    app.install_remote_rows(REMOTE_PATH.into(), vec![remote]);
    app.dual = false;
    app.active = 1;
    let entry = app.panes[1].current().unwrap();
    assert_eq!(entry.kind, fs::EntryKind::Unknown);
    assert!(entry.identity.is_none());
    assert!(entry.created.is_none());
    assert!(entry.accessed.is_none());
    let rendered: String = row(&app).iter().map(Cell::symbol).collect();
    assert!(rendered.contains("?  unknown"));
    assert_eq!(rendered.matches("unavailable").count(), 4, "{rendered}");
}

#[test]
fn raw_remote_names_distinguish_equal_display_text_through_sort_and_filter() {
    let mut app = App::new("/local".into());
    app.install_remote_rows(
        REMOTE_PATH.into(),
        vec![
            entry(b"name-\xff", FileAttributes::default(), 0),
            entry(b"name-\xfe", FileAttributes::default(), 1),
        ],
    );
    let pane = &mut app.panes[1];
    assert_eq!(pane.remote_path(), Some(REMOTE_PATH));
    assert_eq!(pane.entries[0].name, pane.entries[1].name);
    pane.move_cursor(1, 1);
    assert_eq!(pane.scroll, 1);
    assert_eq!(
        pane.current().unwrap().remote_name(),
        Some(b"name-\xff".as_slice())
    );
    pane.set_filter("ame".into());
    pane.sort.sort = Sort::Unsorted;
    pane.resort();
    assert_eq!(pane.cursor_index(), Some(0));
    let selected = pane.current().unwrap();
    assert_eq!(
        selected.remote_path(),
        Some(b"/remote-\xff/name-\xff".as_slice())
    );
    assert!(selected.path.as_os_str().is_empty());
}

#[test]
fn remote_rows_cannot_dispatch_native_work_and_local_navigation_still_works() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("subdir");
    std::fs::create_dir(&child).unwrap();
    let mut app = local_app(root.path());
    app.install_remote_rows(
        REMOTE_PATH.into(),
        vec![entry(
            b"subdir",
            FileAttributes {
                permissions: Some(0o040750),
                ..Default::default()
            },
            0,
        )],
    );
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 167, 20));
    assert!(geometry.panes.iter().all(Option::is_some));
    assert!(
        geometry
            .path_controls
            .iter()
            .all(|control| control.pane == 0)
    );
    controller.dispatch(Action::FocusPane, &geometry);
    assert_remote_actions_blocked(&mut controller, &geometry);
    controller.dispatch(Action::FocusPane, &geometry);
    controller.dispatch(Action::MoveQuick, &geometry);
    assert!(controller.app.operations.outbox.is_empty());
    let requests = controller.dispatch(Action::Enter, &geometry);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].pane, 0);
    assert_eq!(requests[0].path, child);
}

fn assert_remote_actions_blocked(controller: &mut Controller, geometry: &ui::Geometry) {
    assert_eq!(controller.app.active, 1);
    for action in [
        Action::Enter,
        Action::Refresh,
        Action::FullPath,
        Action::Metadata,
        Action::Count,
        Action::CreateFile,
        Action::Delete,
        Action::CopyQuick,
        Action::SaveQuit,
    ] {
        assert!(
            controller.dispatch(action, geometry).is_empty(),
            "{action:?}"
        );
    }
    assert!(controller.app.operations.dialog.is_none());
    assert!(controller.app.operations.outbox.is_empty());
    assert!(!controller.quit);
    assert!(controller.app.panes[1].eligible().is_empty());
    assert!(controller.app.request_metadata(1).is_none());
}
