//! Metadata values share listing styles while keeping formatting local to each open view.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    fs,
    input::{Bindings, Controller},
    permissions::PermissionDisplay,
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    std::fs::write(&path, "data").unwrap();
    let mut app = App::new(root.path().into());
    let mut entry = fs::inspect(&path);
    entry.mode = Some(0o104754);
    app.details[0].entry = Some(entry);
    app.details[0].path = Some(path);
    app.sidebar = true;
    app.panes[0].sidebar = true;
    app.panes[0].profile = Some("Detailed".into());
    let c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    (root, c)
}

fn draw(app: &App, width: u16, height: u16, level: ColorLevel) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn find(buffer: &Buffer, text: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if let Some(offset) = row.find(text) {
            return (row[..offset].chars().count() as u16, y);
        }
    }
    panic!("missing {text}");
}

fn click(c: &mut Controller, x: u16, y: u16, geometry: &ui::Geometry) {
    assert!(
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            }),
            geometry,
            std::time::Duration::ZERO
        )
        .is_empty()
    );
}

fn visible_value(buffer: &Buffer, geometry: &ui::Geometry, text: &str) -> (u16, u16) {
    let control = geometry
        .metadata_controls
        .iter()
        .enumerate()
        .find_map(|(index, control)| {
            let mut value = String::new();
            for run in &geometry.metadata_controls[index..] {
                if run.action != control.action || value.len() >= text.len() {
                    break;
                }
                value.extend(
                    (run.area.x..run.area.right()).map(|x| buffer[(x, run.area.y)].symbol()),
                );
            }
            (value == text).then_some(control)
        })
        .expect("value runs match visible buffer content");
    (control.area.x, control.area.y)
}

#[path = "metadata_controls/attributes.rs"]
mod attributes;
#[path = "metadata_controls/owner.rs"]
mod owner;
#[path = "metadata_controls/timestamps.rs"]
mod timestamps;
