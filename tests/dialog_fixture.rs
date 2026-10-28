use terminal_file_manager::{
    app::App,
    operations::{Dialog, OperationDialog, Plan},
};

pub fn dialog(plan: Plan) -> App {
    let mut app = App::new("/controlled".into());
    app.operations.dialog = Some(Dialog::Operation(Box::new(OperationDialog {
        plan,
        pane: 0,
        base: "/controlled".into(),
        native_target: "/controlled".into(),
        text: String::new(),
        incoming: String::new(),
        edited: false,
        field: 0,
        options: Default::default(),
        ready: true,
        error: None,
        preview_id: None,
        track_scope: false,
        retry_of: None,
    })));
    app
}
