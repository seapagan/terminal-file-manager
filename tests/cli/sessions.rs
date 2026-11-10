use super::isolated_session_command;

#[test]
fn cli_base_uses_first_explicit_directory_or_cwd() {
    use terminal_file_manager::{session::SessionStore, settings::Settings};
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    for paths in [vec![], vec!["first"], vec!["first", "second"]] {
        let output = isolated_session_command(root.path())
            .arg("--save-session")
            .args(&paths)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        let app = store.load().unwrap().restore(&Settings::default()).unwrap();
        let expected = if paths.is_empty() {
            root.path().to_owned()
        } else {
            root.path().join("first")
        };
        assert!(app.relative_path_base.is_absolute());
        assert_eq!(
            std::fs::canonicalize(app.relative_path_base).unwrap(),
            std::fs::canonicalize(expected).unwrap()
        );
    }
}

#[test]
fn explicit_and_automatic_restore_keep_the_saved_base_instead_of_cwd() {
    use terminal_file_manager::{app::App, session::SessionStore, settings::Settings};
    let root = tempfile::tempdir().unwrap();
    let second = root.path().join("captured-base");
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    let mut app = App::new(root.path().to_owned());
    app.relative_path_base = second.clone();
    store.save(&app).unwrap();
    for explicit in [true, false] {
        let mut command = isolated_session_command(root.path());
        if explicit {
            command.arg("--restore-session");
        } else {
            std::fs::write(
                root.path().join("config/tfm/config.toml"),
                "restore_session=true",
            )
            .unwrap();
        }
        assert!(command.arg("--save-session").status().unwrap().success());
        assert_eq!(
            store
                .load()
                .unwrap()
                .restore(&Settings::default())
                .unwrap()
                .relative_path_base,
            second
        );
    }
}
