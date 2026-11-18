#![cfg(windows)]
//! Native ownership stays independent of enumeration and cached presentation.
use std::sync::atomic::AtomicBool;
use terminal_file_manager::{fs, owner::OwnerDisplay};

fn security_owner_sid(path: &std::path::Path, root: &std::path::Path) -> Option<String> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let powershell = security_oracle_directory()?;
    let mut child = match Command::new(powershell.join("powershell.exe"))
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference = 'Stop'; (Get-Acl -LiteralPath $env:TFM_OWNER_FIXTURE).GetOwner([System.Security.Principal.SecurityIdentifier]).Value"])
        .env("TFM_OWNER_FIXTURE", path).env("PSModulePath", powershell.join("Modules"))
        .env("HOME", root).env("APPDATA", root).env("USERPROFILE", root)
        .current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn() {
            Ok(child) => child,
            Err(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "PowerShell launch failed: {error}");
                eprintln!("Skipping Get-Acl oracle: PowerShell unavailable: {error}");
                return None;
            }
        };
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Windows Get-Acl reference timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(String::from_utf8(output.stdout).unwrap().trim().to_owned())
}

fn security_oracle_directory() -> Option<std::path::PathBuf> {
    let Some(system_root) = std::env::var_os("SystemRoot").filter(|value| !value.is_empty()) else {
        eprintln!("Skipping Get-Acl oracle: SystemRoot unavailable");
        return None;
    };
    let directory = std::path::PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0");
    let manifest =
        directory.join("Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1");
    if let Err(error) = std::fs::metadata(&manifest) {
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::NotFound,
            "PowerShell Security module inspection failed: {error}"
        );
        eprintln!("Skipping Get-Acl oracle: Security module unavailable: {error}");
        return None;
    }
    Some(directory)
}

fn loaded_numeric_view(
    root: &std::path::Path,
    path: &std::path::Path,
) -> (terminal_file_manager::app::App, usize) {
    use terminal_file_manager::app::App;
    let snapshot = fs::list(
        root,
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut app = App::new(root.into());
    for pane in &mut app.panes {
        pane.owner_display = OwnerDisplay::Numeric;
    }
    app.panes[0].columns.owner = true;
    app.complete_listing(0, 1, Ok(snapshot.clone()));
    app.complete_listing(1, 1, Ok(snapshot));
    let index = app.panes[0]
        .entries
        .iter()
        .position(|entry| entry.path == path)
        .unwrap();
    app.panes[0].cursor = Some(path.into());
    app.request_metadata(0).unwrap();
    app.popup = true;
    (app, index)
}

fn rendered_owner_text(app: &terminal_file_manager::app::App) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    use terminal_file_manager::{color::Theme, ui};
    let mut terminal = Terminal::new(TestBackend::new(240, 40)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn native_runtime_updates_listing_popup_and_sidebar_to_the_actual_security_owner() {
    use ratatui::layout::Rect;
    use std::time::{Duration, Instant};
    use terminal_file_manager::{
        owner::windows::{Ownership, runtime::Runtime},
        ui,
    };
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(if directory { "directory" } else { "file" });
        if directory {
            std::fs::create_dir(&path).unwrap();
        } else {
            std::fs::write(&path, b"x").unwrap();
        }
        let Some(expected) = security_owner_sid(&path, root.path()) else {
            return;
        };
        assert!(expected.starts_with("S-1-"));
        let (mut app, index) = loaded_numeric_view(root.path(), &path);
        let geometry = ui::geometry(&app, Rect::new(0, 0, 240, 40));
        let mut runtime = Runtime::new().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while matches!(
            app.panes[0].entries[index].windows_owner,
            Ownership::Pending
        ) {
            runtime.poll(&mut app, &geometry);
            assert!(Instant::now() < deadline, "native ownership timed out");
            std::thread::yield_now();
        }
        assert_eq!(
            app.panes[0].entries[index].owner_display(OwnerDisplay::Numeric),
            expected
        );
        assert_eq!(
            app.details[0].entry.as_ref().unwrap().windows_owner,
            app.panes[0].entries[index].windows_owner
        );
        for popup in [true, false] {
            app.popup = popup;
            app.sidebar = !popup;
            let text = rendered_owner_text(&app);
            assert!(text.contains("Owner:"), "{text}");
            assert!(!text.contains("Owner/group:"));
        }
    }
}

#[test]
fn native_owner_sid_and_known_account_preserve_numeric_identity() {
    use terminal_file_manager::owner::windows::{Name, read_sid, resolve_name};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owner-界");
    std::fs::write(&path, b"x").unwrap();
    let entry = fs::inspect(&path);
    let sid = read_sid(&path, entry.identity.as_ref().unwrap()).unwrap();
    assert!(sid.starts_with("S-1-"), "{sid}");
    assert!(matches!(resolve_name("S-1-5-18"), Name::Resolved(name) if name.contains('\\')));
    assert!(matches!(resolve_name("not a SID"), Name::Failed(_)));
    assert!(matches!(resolve_name("S-1-5-999"), Name::Unmapped));
    std::fs::remove_file(&path).unwrap();
    assert!(read_sid(&path, entry.identity.as_ref().unwrap()).is_err());
}

#[test]
fn cached_sid_distinguishes_pending_unmapped_and_failed_name_resolution() {
    use terminal_file_manager::owner::windows::{Name, Ownership};
    let mut entry = fs::inspect(tempfile::tempdir().unwrap().path());
    for (name, expected) in [
        (Name::Pending, "S-1-5-18 (resolving)"),
        (Name::Unmapped, "S-1-5-18"),
        (Name::Failed("denied".into()), "S-1-5-18 (lookup failed)"),
        (
            Name::Resolved("NT AUTHORITY\\SYSTEM".into()),
            "NT AUTHORITY\\SYSTEM",
        ),
    ] {
        entry.windows_owner = Ownership::Sid {
            sid: "S-1-5-18".into(),
            name,
        };
        assert_eq!(entry.owner_display(OwnerDisplay::Names), expected);
        assert_eq!(entry.owner_display(OwnerDisplay::Numeric), "S-1-5-18");
    }
    entry.windows_owner = Ownership::Unavailable("denied".into());
    assert_eq!(entry.owner_display(OwnerDisplay::Names), "unavailable");
}

#[test]
fn no_follow_query_rejects_a_different_object_and_accepts_the_link_itself() {
    use terminal_file_manager::owner::windows::read_sid;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("target");
    let link = root.path().join("link");
    std::fs::write(&file, b"x").unwrap();
    if let Err(error) = std::os::windows::fs::symlink_file(&file, &link) {
        assert_eq!(
            error.raw_os_error(),
            Some(1314),
            "unexpected symlink failure"
        );
        eprintln!("File symlink creation unavailable: {error}");
        return;
    }
    let target = fs::inspect_identity(&file);
    let symlink = fs::inspect_identity(&link);
    assert!(read_sid(&link, target.identity.as_ref().unwrap()).is_err());
    assert!(
        read_sid(&link, symlink.identity.as_ref().unwrap())
            .unwrap()
            .starts_with("S-1-")
    );
}

#[test]
fn enumeration_marks_ownership_pending_without_claiming_a_lookup_failure() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"x").unwrap();
    let snapshot = fs::list(
        root.path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    for entry in snapshot.entries {
        assert!(entry.owner.is_none());
        assert_eq!(entry.owner_display(OwnerDisplay::Names), "resolving");
        assert_eq!(entry.owner_display(OwnerDisplay::Numeric), "resolving");
    }
}

#[test]
fn missing_metadata_reports_unavailable_ownership_instead_of_pending_work() {
    let root = tempfile::tempdir().unwrap();
    let entry = fs::inspect(&root.path().join("missing"));
    assert_eq!(entry.owner_display(OwnerDisplay::Names), "unavailable");
    assert!(entry.windows_owner.error().is_some());
}
