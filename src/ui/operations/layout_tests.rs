use super::layout::Layout;
use crate::{
    app::App,
    color::Theme,
    operations::{Dialog, Operation, OperationDialog, Plan},
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn scroll_cues_are_interior_and_all_geometry_stays_centred_after_resize() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("selected");
    std::fs::write(&file, b"kept").unwrap();
    let mut plan = Plan::paths(Operation::Copy, vec![file], Some(root.path().into())).unwrap();
    plan.operands = vec![plan.operands[0].clone(); 50];
    let mut app = App::new(root.path().into());
    app.operations.dialog = Some(Dialog::Operation(Box::new(OperationDialog {
        plan,
        pane: 0,
        base: root.path().into(),
        native_target: root.path().into(),
        text: "target".into(),
        incoming: String::new(),
        edited: false,
        field: 0,
        options: Default::default(),
        ready: true,
        preview_id: None,
        track_scope: false,
        retry_of: None,
        error: None,
    })));
    for width in [20, 40, 80, 120] {
        for height in [12, 13, 20, 30, 50] {
            let terminal = Rect::new(0, 0, width, height);
            let layout = Layout::new(&app, terminal);
            let top = layout.outer.y;
            let bottom = height - 2 - layout.outer.bottom();
            assert!(top.abs_diff(bottom) <= 1);
            if height >= 30 {
                assert!(layout.outer.height <= height * 3 / 4);
            }
            assert!(layout.shell.cue.bottom() < layout.outer.bottom());
            assert!(layout.shell.cue.y >= layout.body.bottom());
            assert_buffer(&app, &layout, width, height);
        }
    }
}

fn assert_buffer(app: &App, layout: &Layout, width: u16, height: u16) {
    let mut output = Terminal::new(TestBackend::new(width, height)).unwrap();
    output
        .draw(|f| {
            crate::ui::render(f, app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = output.backend().buffer();
    if layout.body.height == 0 && layout.shell.cue.height > 0 {
        let cue: String = (layout.shell.cue.x..layout.shell.cue.right())
            .map(|x| buffer[(x, layout.shell.cue.y)].symbol())
            .collect();
        assert!(cue.contains("0/50") && cue.contains("Resize"));
    }
    assert_eq!(
        buffer[(layout.outer.x, layout.outer.bottom() - 1)].symbol(),
        "└"
    );
    assert_eq!(
        buffer[(layout.outer.right() - 1, layout.outer.bottom() - 1)].symbol(),
        "┘"
    );
    for x in layout.outer.x + 1..layout.outer.right() - 1 {
        assert_eq!(buffer[(x, layout.outer.bottom() - 1)].symbol(), "─");
    }
    for action in &layout.shell.actions {
        assert_eq!(buffer[(action.area.x, action.area.y)].symbol(), "[");
        assert_eq!(
            buffer[(action.area.right() - 1, action.area.y)].symbol(),
            "]"
        );
        assert_eq!(
            layout.shell.hit(action.area.x, action.area.y),
            Some(&action.action.intent)
        );
    }
}
