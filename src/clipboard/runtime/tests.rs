use super::*;
use std::time::{Duration, Instant};

fn request() -> CopyRequest {
    CopyRequest {
        text: "界 file;$(literal)".into(),
        scope: "1 full path".into(),
    }
}

fn wait(runtime: &mut Runtime, terminal: &mut dyn Write) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = runtime.poll(terminal) {
            return result;
        }
        assert!(Instant::now() < deadline, "worker result");
        std::thread::yield_now();
    }
}

#[test]
fn worker_does_not_block_ui_or_overwrite_a_running_request_and_completes_once() {
    let mut runtime = Runtime::default();
    let (ready, received) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    runtime
        .start_using(request(), move |text| {
            ready.send(text).unwrap();
            blocked.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(Delivery::Copied)
        })
        .unwrap();
    assert_eq!(
        received.recv_timeout(Duration::from_secs(10)).unwrap(),
        request().text
    );
    assert!(runtime.busy());
    assert!(runtime.poll(&mut Vec::new()).is_none());
    assert!(
        runtime
            .start_using(request(), |_| panic!("duplicate provider"))
            .is_err()
    );
    release.send(()).unwrap();
    assert_eq!(
        wait(&mut runtime, &mut Vec::new()).unwrap(),
        "Copied 1 full path"
    );
    assert!(!runtime.busy());
    assert!(runtime.poll(&mut Vec::new()).is_none());
}

#[test]
fn osc52_is_emitted_by_ui_owner_and_only_claims_sent_once() {
    let mut runtime = Runtime::default();
    runtime
        .start_using(request(), |text| {
            Ok(Delivery::Terminal(super::super::encode_osc52(
                text.as_bytes(),
            )?))
        })
        .unwrap();
    let mut terminal = Vec::new();
    let message = wait(&mut runtime, &mut terminal).unwrap();
    assert_eq!(
        terminal,
        super::super::encode_osc52(request().text.as_bytes()).unwrap()
    );
    assert!(message.starts_with("Sent ") && message.contains("acceptance unconfirmed"));
    assert!(runtime.poll(&mut terminal).is_none());
}

#[test]
fn provider_and_terminal_failures_return_errors_without_false_success() {
    let mut runtime = Runtime::default();
    runtime
        .start_using(request(), |_| Err(ClipboardError("provider failed".into())))
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
