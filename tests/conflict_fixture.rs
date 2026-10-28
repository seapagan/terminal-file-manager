use colored_text::ColorLevel;

use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{
    sync::mpsc::{Receiver, sync_channel},
    time::{Duration, UNIX_EPOCH},
};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::Entry,
    operations::{Decision, Event, PendingConflict},
    ui,
};
pub fn fixture() -> (tempfile::TempDir, App, Receiver<Decision>) {
    let root = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for (name, bytes) in [("source", 22), ("destination", 53)] {
        let directory = root.path().join(name);
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("conflict.txt");
        std::fs::write(&path, vec![b'x'; bytes]).unwrap();
        let mut entry = terminal_file_manager::fs::inspect(&path);
        entry.modified = Some(
            UNIX_EPOCH
                + Duration::new(
                    1793038260,
                    if bytes == 22 { 50_000_000 } else { 90_000_000 },
                ),
        );
        entries.push(entry);
    }
    let mut app = App::new(root.path().into());
    let (reply, response) = sync_channel(1);
    app.operations.conflicts.push_back(PendingConflict {
        id: 2,
        choice: 0,
        all: false,
        reply,
        event: Event::Conflict {
            source: Box::new(entries.remove(0)),
            destination: Box::new(entries.remove(0)),
            newer_available: true,
        },
    });
    (root, app, response)
}
pub fn draw(
    app: &App,
    width: u16,
    height: u16,
    level: ColorLevel,
) -> (Terminal<TestBackend>, ui::Geometry, String) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let g = ui::geometry(app, Rect::new(0, 0, width, height));
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    (terminal, g, text)
}
pub fn edit(app: &mut App, f: impl FnOnce(&mut Entry, &mut Entry, &mut bool)) {
    if let Event::Conflict {
        source,
        destination,
        newer_available,
    } = &mut app.operations.conflicts.front_mut().unwrap().event
    {
        f(source, destination, newer_available)
    }
}
