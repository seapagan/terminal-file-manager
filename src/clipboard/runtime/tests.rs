use super::*;
use std::time::{Duration, Instant};

const TEXT: &str = "a 界 file;$(literal)";

fn root() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(TEXT), "").unwrap();
    root
}

fn app(root: &std::path::Path) -> crate::app::App {
    let mut app = crate::app::App::new(root.into());
    let listing = crate::fs::list(root, &Default::default(), &Default::default()).unwrap();
    app.complete_listing(0, 1, Ok(listing));
    app
}

fn completion(app: &mut crate::app::App, source: &'static str) -> crate::path_copy::Outcome {
    use crate::workflow::{Input, Progress, Target};
    let mut flow = crate::path_copy::open(app, Input::Click(Target::Choice("path-copy"))).unwrap();
    let mut result = flow.input(Input::Click(Target::Choice(source)));
    if source != "current-base" && source != "opposite-base" {
        result = flow.input(Input::Click(Target::Choice("name")));
    }
    let Progress::Complete(done) = result else {
        panic!("workflow completion")
    };
    crate::path_copy::complete(done, app).unwrap()
}

fn request(root: &std::path::Path) -> CopyRequest {
    let crate::path_copy::Outcome::Copy(request) = completion(&mut app(root), "highlighted") else {
        panic!("copy request")
    };
    request
}

fn wait(runtime: &mut Runtime, terminal: &mut dyn Write) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = runtime.poll(terminal) {
            return result.map(|done| match done {
                Completion::Message(message) => message,
                Completion::Base(_) => panic!("unexpected base result"),
            });
        }
        assert!(Instant::now() < deadline, "worker result");
        std::thread::yield_now();
    }
}

#[test]
fn worker_does_not_block_ui_or_overwrite_a_running_request_and_completes_once() {
    let root = root();
    let mut runtime = Runtime::default();
    let (ready, received) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    runtime
        .start_using(request(root.path()), move |text| {
            ready.send(text).unwrap();
            blocked.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(Delivery::Copied)
        })
        .unwrap();
    assert_eq!(
        received.recv_timeout(Duration::from_secs(10)).unwrap(),
        TEXT
    );
    assert!(runtime.busy());
    assert!(runtime.poll(&mut Vec::new()).is_none());
    assert!(
        runtime
            .start_using(request(root.path()), |_| panic!("duplicate provider"))
            .is_err()
    );
    release.send(()).unwrap();
    assert_eq!(
        wait(&mut runtime, &mut Vec::new()).unwrap(),
        "Copied 1 name"
    );
    assert!(!runtime.busy());
    assert!(runtime.poll(&mut Vec::new()).is_none());
}

#[test]
fn osc52_is_emitted_by_ui_owner_and_only_claims_sent_once() {
    let root = root();
    let mut runtime = Runtime::default();
    runtime
        .start_using(request(root.path()), |text| {
            Ok(Delivery::Terminal(super::super::encode_osc52(
                text.as_bytes(),
            )?))
        })
        .unwrap();
    let mut terminal = Vec::new();
    let message = wait(&mut runtime, &mut terminal).unwrap();
    assert_eq!(
        terminal,
        super::super::encode_osc52(TEXT.as_bytes()).unwrap()
    );
    assert!(message.starts_with("Sent ") && message.contains("acceptance unconfirmed"));
    assert!(runtime.poll(&mut terminal).is_none());
}

#[test]
fn provider_and_terminal_failures_return_errors_without_false_success() {
    let root = root();
    let mut runtime = Runtime::default();
    runtime
        .start_using(request(root.path()), |_| {
            Err(ClipboardError("provider failed".into()))
        })
        .unwrap();
    assert_eq!(
        wait(&mut runtime, &mut Vec::new()).unwrap_err(),
        "provider failed"
    );
    let error = finish(
        Delivery::Terminal(vec![1]),
        "name",
        &mut super::super::tests::FailingWriter,
    )
    .unwrap_err();
    assert!(error.contains("Cannot send OSC 52"));
}

#[test]
fn slow_native_inspection_keeps_polling_responsive_and_blocks_duplicate_work() {
    let root = root();
    let mut runtime = Runtime::default();
    let (ready, received) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let mut first = true;
    runtime
        .start_inspecting(
            request(root.path()),
            move |path| {
                if first {
                    first = false;
                    ready.send(()).unwrap();
                    blocked.recv_timeout(Duration::from_secs(10)).unwrap();
                }
                crate::fs::inspect_identity(path)
            },
            |text| {
                assert_eq!(text, TEXT);
                Ok(Delivery::Copied)
            },
        )
        .unwrap();
    received.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(runtime.poll(&mut Vec::new()).is_none());
    assert!(
        runtime
            .start_using(request(root.path()), |_| panic!("duplicate write"))
            .is_err()
    );
    release.send(()).unwrap();
    assert_eq!(
        wait(&mut runtime, &mut Vec::new()).unwrap(),
        "Copied 1 name"
    );
}

#[test]
fn stale_last_mark_rejects_the_entire_batch_before_provider_or_terminal_output() {
    let root = root();
    std::fs::write(root.path().join("z-last"), "old").unwrap();
    let mut app = app(root.path());
    app.panes[0].select_all();
    let crate::path_copy::Outcome::Copy(request) = completion(&mut app, "marked") else {
        panic!("copy")
    };
    std::fs::rename(root.path().join("z-last"), root.path().join("old")).unwrap();
    std::fs::write(root.path().join("z-last"), "replacement").unwrap();
    let mut runtime = Runtime::default();
    runtime
        .start_using(request, |_| panic!("stale batch must never reach provider"))
        .unwrap();
    let mut terminal = Vec::new();
    assert!(
        wait(&mut runtime, &mut terminal)
            .unwrap_err()
            .contains("replaced")
    );
    assert!(terminal.is_empty());
}

#[test]
fn inspection_errors_with_identity_evidence_cannot_write_clipboard_text() {
    let root = root();
    let reached = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let provider_reached = reached.clone();
    let mut runtime = Runtime::default();
    runtime
        .start_inspecting(
            request(root.path()),
            |path| {
                let mut entry = crate::fs::inspect_identity(path);
                entry.error = Some("metadata unavailable".into());
                entry
            },
            move |_| {
                provider_reached.store(true, std::sync::atomic::Ordering::SeqCst);
                Err(ClipboardError("provider was reached".into()))
            },
        )
        .unwrap();
    assert_eq!(
        wait(&mut runtime, &mut Vec::new()).unwrap_err(),
        "Current directory was removed or replaced; refresh and start again"
    );
    assert!(!reached.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn worker_disconnect_reports_failure_and_releases_the_single_request_lane() {
    let root = root();
    let mut runtime = Runtime::default();
    runtime
        .start_using(request(root.path()), |_| panic!("worker failure"))
        .unwrap();
    assert!(
        wait(&mut runtime, &mut Vec::new())
            .unwrap_err()
            .contains("without a result")
    );
    assert!(!runtime.busy());
}

#[test]
fn slow_base_validation_rechecks_app_context_before_changing_the_base() {
    let root = root();
    let mut app = app(root.path());
    let original = app.relative_path_base.clone();
    let crate::path_copy::Outcome::BaseSet(request) = completion(&mut app, "current-base") else {
        panic!("base")
    };
    let mut runtime = Runtime::default();
    let (ready, received) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let mut first = true;
    runtime
        .start_base_inspecting(request, move |path| {
            if first {
                first = false;
                ready.send(()).unwrap();
                blocked.recv_timeout(Duration::from_secs(10)).unwrap();
            }
            crate::fs::inspect_identity(path)
        })
        .unwrap();
    received.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(runtime.poll(&mut Vec::new()).is_none());
    app.request_listing(0, root.path().join("elsewhere"));
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = runtime.poll(&mut Vec::new()) {
            let Ok(Completion::Base(validated)) = result else {
                panic!("validated base")
            };
            assert!(validated.apply(&mut app).is_err());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(app.relative_path_base, original);
}
