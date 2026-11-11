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

#[cfg(unix)]
#[test]
fn explicit_and_automatic_restore_work_from_a_deleted_cwd() {
    use terminal_file_manager::{app::App, session::SessionStore, settings::Settings};
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    let mut app = App::new(root.path().to_owned());
    app.relative_path_base = root.path().join("saved-base");
    store.save(&app).unwrap();
    for explicit in [true, false] {
        let deleted = root.path().join("deleted-cwd");
        std::fs::create_dir(&deleted).unwrap();
        if !explicit {
            std::fs::write(
                root.path().join("config/tfm/config.toml"),
                "restore_session=true",
            )
            .unwrap();
        }
        let mut command = deleted_cwd_command(root.path(), &deleted);
        if explicit {
            command.arg("--restore-session");
        }
        let output = command.arg("--save-session").output().unwrap();
        assert!(!deleted.exists());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let restored = store.load().unwrap().restore(&Settings::default()).unwrap();
        assert_eq!(restored.panes[0].path, root.path());
        assert_eq!(restored.panes[1].path, root.path());
        assert_eq!(restored.relative_path_base, app.relative_path_base);
    }
}

#[cfg(unix)]
fn deleted_cwd_command(root: &std::path::Path, cwd: &std::path::Path) -> std::process::Command {
    let isolated = isolated_session_command(root);
    let mut command = std::process::Command::new("sh");
    command
        .current_dir(cwd)
        .args([
            "-c",
            "rmdir \"$1\" || exit; shift; exec \"$@\"",
            "tfm-deleted-cwd",
        ])
        .arg(cwd)
        .arg(isolated.get_program());
    for (key, value) in isolated.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    command
}
