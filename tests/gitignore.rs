//! Git oracle fixtures, adapted from lsplus's MIT-licensed gitignore regressions.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
};
use terminal_file_manager::{
    app::{App, IgnoreMode},
    fs::{IgnoreContext, Snapshot, list},
};

struct Repository {
    temp: tempfile::TempDir,
    path: PathBuf,
    global: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("repo");
        let global = temp.path().join("global-ignore");
        fs::create_dir(&path).unwrap();
        fs::write(&global, "").unwrap();
        let repo = Self { temp, path, global };
        repo.run(&repo.path, &["init", "--quiet"]);
        repo
    }

    fn command(&self, directory: &Path) -> Command {
        let mut command = Command::new("git");
        command.current_dir(directory).env_clear();
        for key in ["PATH", "SystemRoot", "SYSTEMROOT", "WINDIR", "TMP", "TEMP"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .env("HOME", self.temp.path())
            .env("USERPROFILE", self.temp.path())
            .env("XDG_CONFIG_HOME", self.temp.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.temp.path().join("absent-config"))
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
            ])
            .arg(format!("core.excludesFile={}", self.global.display()));
        command
    }

    fn run(&self, directory: &Path, args: &[&str]) {
        let output = self.command(directory).args(args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {:?}", output);
    }

    fn compare(&self, root: &Path, relative: impl AsRef<Path>, expected: Option<bool>) {
        let relative = relative.as_ref();
        let path = root.join(relative);
        let mut child = self
            .command(root)
            .args([
                "check-ignore",
                "--no-index",
                "--verbose",
                "--non-matching",
                "-z",
                "--stdin",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = relative.as_os_str().as_encoded_bytes().to_vec();
        input.push(0);
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(matches!(output.status.code(), Some(0 | 1)), "{output:?}");
        let fields: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
        assert_eq!(fields.len(), 5, "{output:?}");
        let oracle = (!fields[2].is_empty()).then(|| !fields[2].starts_with(b"!"));
        assert_eq!(oracle, expected, "Git: {relative:?}");
        let context = IgnoreContext {
            global_file: Some(self.global.clone()),
            discover_global: false,
        };
        let snapshot = list(path.parent().unwrap(), &context, &AtomicBool::new(false)).unwrap();
        assert!(snapshot.errors.is_empty(), "{:?}", snapshot.errors);
        let entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .unwrap();
        assert_eq!(
            entry.ignored,
            Some(oracle.unwrap_or(false)),
            "TFM: {relative:?}"
        );
        assert_ignore_modes(snapshot, &path, oracle.unwrap_or(false));
    }
}

#[test]
fn nested_parent_and_source_precedence_match_git() {
    let repo = Repository::new();
    fs::create_dir(repo.path.join("nested")).unwrap();
    fs::create_dir(repo.path.join("blocked")).unwrap();
    fs::create_dir(repo.path.join("dir-only")).unwrap();
    fs::write(&repo.global, "AGENTS.md\n*.log\ninfo-wins\n!global-loses\n").unwrap();
    fs::write(
        repo.path.join(".git/info/exclude"),
        "*.diff\ncodacy*.json\n!info-wins\nglobal-loses\n!local-wins\n",
    )
    .unwrap();
    fs::write(
        repo.path.join(".gitignore"),
        "!keep.log\nlocal-wins\nblocked/\ndir-only/\nfile-only/\n",
    )
    .unwrap();
    fs::write(
        repo.path.join("nested/.gitignore"),
        "!local-wins\nkeep.log\n!again.log\n",
    )
    .unwrap();
    fs::write(repo.path.join("blocked/.gitignore"), "!keep.log\n").unwrap();
    let cases = [
        ("AGENTS.md", Some(true)),
        ("branch.diff", Some(true)),
        ("codacy-pr-1.json", Some(true)),
        ("plain", None),
        ("gone.log", Some(true)),
        ("keep.log", Some(false)),
        ("info-wins", Some(false)),
        ("global-loses", Some(true)),
        ("local-wins", Some(true)),
        ("nested/local-wins", Some(false)),
        ("nested/keep.log", Some(true)),
        ("nested/again.log", Some(false)),
        ("blocked/keep.log", Some(true)),
        ("dir-only", Some(true)),
        ("dir-only/child", Some(true)),
        ("file-only", None),
    ];
    for (path, _) in cases {
        if path != "dir-only" {
            fs::write(repo.path.join(path), "fixture").unwrap();
        }
    }
    for (path, expected) in cases {
        repo.compare(&repo.path, path, expected);
    }
}

#[test]
fn refresh_reloads_each_source_without_a_lifetime_cache() {
    let repo = Repository::new();
    fs::write(&repo.global, "gone.log\n").unwrap();
    fs::write(repo.path.join("gone.log"), "fixture").unwrap();
    fs::write(repo.path.join("plain"), "fixture").unwrap();
    repo.compare(&repo.path, "gone.log", Some(true));
    fs::write(repo.path.join(".gitignore"), "!gone.log\n").unwrap();
    repo.compare(&repo.path, "gone.log", Some(false));
    fs::write(&repo.global, "plain\n").unwrap();
    repo.compare(&repo.path, "plain", Some(true));
    fs::write(repo.path.join(".git/info/exclude"), "!plain\n").unwrap();
    repo.compare(&repo.path, "plain", Some(false));
}

#[test]
fn real_linked_worktree_uses_common_excludes_and_native_paths() {
    let repo = Repository::new();
    repo.run(
        &repo.path,
        &["commit", "--quiet", "--allow-empty", "-m", "fixture"],
    );
    let linked = repo.temp.path().join("linked 界");
    let output = repo
        .command(&repo.path)
        .args(["worktree", "add", "--quiet", "-b", "linked"])
        .arg(&linked)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    fs::write(repo.path.join(".git/info/exclude"), "excluded\n").unwrap();
    fs::write(linked.join("excluded"), "fixture").unwrap();
    repo.compare(&linked, "excluded", Some(true));
    fs::write(linked.join(".gitignore"), "!excluded\n").unwrap();
    repo.compare(&linked, "excluded", Some(false));
    // Git chooses the worktree administrative name; retain its exact native spelling.
    let admin = fs::read_dir(repo.path.join(".git/worktrees"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(
        linked.join(".git"),
        format!(
            "gitdir: ../repo/.git/worktrees/{}\n",
            admin.file_name().unwrap().to_str().unwrap()
        ),
    )
    .unwrap();
    repo.compare(&linked, "excluded", Some(false));
    fs::write(
        admin.join("commondir"),
        format!("{}\n", repo.path.join(".git").display()),
    )
    .unwrap();
    repo.compare(&linked, "excluded", Some(false));
}

#[test]
fn invalid_or_unreadable_rule_sources_are_unknown_not_unignored() {
    let repo = Repository::new();
    fs::write(repo.path.join("visible"), "fixture").unwrap();
    for rule in [
        repo.path.join(".gitignore"),
        repo.path.join(".git/info/exclude"),
        repo.global.clone(),
    ] {
        let previous = fs::read(&rule).ok();
        fs::write(&rule, "{a,b\n").unwrap();
        assert_unknown(&repo);
        fs::remove_file(&rule).unwrap();
        fs::create_dir(&rule).unwrap();
        assert_unknown(&repo);
        fs::remove_dir(&rule).unwrap();
        if let Some(previous) = previous {
            fs::write(&rule, previous).unwrap();
        }
    }
}

fn assert_unknown(repo: &Repository) {
    let context = IgnoreContext {
        global_file: Some(repo.global.clone()),
        discover_global: false,
    };
    let snapshot = list(&repo.path, &context, &AtomicBool::new(false)).unwrap();
    assert!(!snapshot.errors.is_empty());
    assert!(snapshot.entries.iter().all(|entry| entry.ignored.is_none()));
}

#[cfg(target_os = "linux")]
#[test]
fn raw_names_remain_distinct_when_matching_git_rules() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let repo = Repository::new();
    fs::write(repo.path.join(".gitignore"), "*.log\n").unwrap();
    for byte in [0xfe, 0xff] {
        let name = PathBuf::from(OsString::from_vec(vec![b'n', byte, b'.', b'l', b'o', b'g']));
        fs::write(repo.path.join(&name), "fixture").unwrap();
        repo.compare(&repo.path, name, Some(true));
    }
}

fn assert_ignore_modes(snapshot: Snapshot, path: &Path, ignored: bool) {
    let mut app = App::new(snapshot.path.clone());
    app.complete_listing(0, 1, Ok(snapshot));
    let pane = &mut app.panes[0];
    pane.show_hidden = true;
    for (mode, visible) in [(IgnoreMode::Dim, true), (IgnoreMode::Hide, !ignored)] {
        pane.ignore = mode;
        pane.refilter();
        assert_eq!(
            pane.visible
                .iter()
                .any(|index| pane.entries[*index].path == path),
            visible,
            "{mode:?}: {path:?}"
        );
    }
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link).unwrap();
}

fn directory_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

fn remove_directory_link(link: &Path) {
    #[cfg(unix)]
    fs::remove_file(link).unwrap();
    #[cfg(windows)]
    fs::remove_dir(link).unwrap();
}

#[test]
fn git_directory_links_and_pointer_files_follow_git_without_hiding_broken_links() {
    let repo = Repository::new();
    let dot = repo.path.join(".git");
    let admin = repo.temp.path().join("linked-administration");
    fs::rename(&dot, &admin).unwrap();
    fs::write(admin.join("info/exclude"), "info-victim\n").unwrap();
    fs::write(&repo.global, "global-victim\n").unwrap();
    fs::write(repo.path.join(".gitignore"), "local-victim\n").unwrap();
    for name in ["info-victim", "global-victim", "local-victim"] {
        fs::write(repo.path.join(name), "fixture").unwrap();
    }
    // CreateSymbolicLinkW stores relative targets verbatim; use native separators.
    directory_link(&Path::new("..").join("linked-administration"), &dot);
    assert!(fs::metadata(&dot).unwrap().is_dir());
    assert_eq!(
        fs::canonicalize(&dot).unwrap(),
        fs::canonicalize(&admin).unwrap()
    );
    repo.run(&repo.path, &["rev-parse", "--show-toplevel"]);
    for name in ["info-victim", "global-victim", "local-victim"] {
        repo.compare(&repo.path, name, Some(true));
    }
    remove_directory_link(&dot);
    directory_link(&Path::new("..").join("missing-administration"), &dot);
    let output = repo
        .command(&repo.path)
        .args(["check-ignore", "info-victim"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(128));
    assert_unknown(&repo);
    remove_directory_link(&dot);
    fs::write(&dot, "gitdir: ../linked-administration\n").unwrap();
    for name in ["info-victim", "global-victim", "local-victim"] {
        repo.compare(&repo.path, name, Some(true));
    }
}

#[cfg(windows)]
#[test]
fn windows_slash_directory_link_is_a_malformed_oracle_fixture() {
    let repo = Repository::new();
    let dot = repo.path.join(".git");
    fs::rename(&dot, repo.temp.path().join("linked-administration")).unwrap();
    // Reproduce the old fixture: Win32 does not normalize slashes in this target.
    directory_link(Path::new("../linked-administration"), &dot);
    assert!(fs::symlink_metadata(&dot).unwrap().file_type().is_symlink());
    assert!(fs::metadata(&dot).is_err());
    let output = repo
        .command(&repo.path)
        .args(["check-ignore", "info-victim"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(128), "{output:?}");
    assert_unknown(&repo);
    remove_directory_link(&dot);
}

#[test]
fn symlinked_root_ignore_is_skipped_and_regular_replacement_is_reloaded() {
    let repo = Repository::new();
    let rules = repo.path.join("rules");
    let ignore = repo.path.join(".gitignore");
    fs::write(&rules, "victim\n").unwrap();
    fs::write(repo.path.join("victim"), "fixture").unwrap();
    symlink_file(Path::new("rules"), &ignore);
    repo.compare(&repo.path, "victim", None);
    fs::remove_file(&ignore).unwrap();
    fs::write(&ignore, "victim\n").unwrap();
    repo.compare(&repo.path, "victim", Some(true));
    fs::remove_file(&ignore).unwrap();
    symlink_file(Path::new("rules"), &ignore);
    fs::remove_file(&rules).unwrap();
    repo.compare(&repo.path, "victim", None);
}

#[test]
fn nested_symlinked_ignore_cannot_override_parent_or_child_rules() {
    let repo = Repository::new();
    fs::create_dir_all(repo.path.join("nested/deep")).unwrap();
    fs::write(repo.path.join(".gitignore"), "*.log\n").unwrap();
    fs::write(repo.path.join("rules"), "!keep.log\nvictim\n{a,b\n").unwrap();
    symlink_file(Path::new("../rules"), &repo.path.join("nested/.gitignore"));
    fs::write(
        repo.path.join("nested/deep/.gitignore"),
        "!keep.log\nlocal\n",
    )
    .unwrap();
    for (path, expected) in [
        ("nested/keep.log", Some(true)),
        ("nested/victim", None),
        ("nested/deep/victim", None),
        ("nested/deep/keep.log", Some(false)),
        ("nested/deep/local", Some(true)),
    ] {
        fs::write(repo.path.join(path), "fixture").unwrap();
        repo.compare(&repo.path, path, expected);
    }
}

#[test]
fn global_and_info_exclude_symlinks_keep_their_distinct_policy() {
    let repo = Repository::new();
    let global_rules = repo.temp.path().join("global-rules");
    let info_rules = repo.temp.path().join("info-rules");
    fs::write(&global_rules, "global-victim\nglobal-keep\n").unwrap();
    fs::write(&info_rules, "info-victim\ninfo-keep\n").unwrap();
    fs::remove_file(&repo.global).unwrap();
    symlink_file(&global_rules, &repo.global);
    fs::remove_file(repo.path.join(".git/info/exclude")).unwrap();
    symlink_file(&info_rules, &repo.path.join(".git/info/exclude"));
    fs::write(repo.path.join(".gitignore"), "!global-keep\n!info-keep\n").unwrap();
    for (path, expected) in [
        ("global-victim", Some(true)),
        ("info-victim", Some(true)),
        ("global-keep", Some(false)),
        ("info-keep", Some(false)),
    ] {
        fs::write(repo.path.join(path), "fixture").unwrap();
        repo.compare(&repo.path, path, expected);
    }
}
