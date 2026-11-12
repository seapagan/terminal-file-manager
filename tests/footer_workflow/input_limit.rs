use super::{flow, key};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use terminal_file_manager::workflow::{Choice, Flow, Input, MAX_INPUTS, Progress, Stage};

#[test]
fn completion_at_input_limit_retains_activation_detours_and_unsupported_events() {
    let mut flow = flow();
    let mut expected = vec![key(KeyCode::Char('x'))];
    for input in [key(KeyCode::Enter), key(KeyCode::Backspace)] {
        assert!(matches!(flow.input(input.clone()), Progress::Active));
        expected.push(input);
    }
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        let mut event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        event.kind = kind;
        let input = Input::Key(event);
        assert!(matches!(flow.input(input.clone()), Progress::Active));
        expected.push(input);
    }
    while expected.len() < MAX_INPUTS - 2 {
        let input = key(KeyCode::Up);
        assert!(matches!(flow.input(input.clone()), Progress::Active));
        expected.push(input);
    }
    assert!(matches!(flow.input(key(KeyCode::Enter)), Progress::Active));
    expected.extend([key(KeyCode::Enter), key(KeyCode::Enter)]);
    let Progress::Complete((_, inputs, choices)) = flow.input(key(KeyCode::Enter)) else {
        panic!("completion at the exact input limit")
    };
    assert_eq!(inputs, expected);
    assert_eq!(inputs.len(), MAX_INPUTS);
    assert_eq!(choices, ["dummy", "first", "finish"]);
    assert!(matches!(
        flow.input(key(KeyCode::Enter)),
        Progress::Inactive
    ));
}

#[test]
fn event_after_input_limit_terminates_without_callback_or_retained_context() {
    let called = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = called.clone();
    let mut flow = Flow::new(
        "dummy",
        "DUMMY",
        key(KeyCode::Char('x')),
        (),
        vec![Stage::new(
            "Finish",
            vec![Choice::new("finish", 'f', ["Finish"; 3])],
            "finish",
        )],
        move |_| count.set(count.get() + 1),
    )
    .unwrap();
    for _ in 1..MAX_INPUTS {
        assert!(matches!(flow.input(key(KeyCode::Up)), Progress::Active));
    }
    assert!(matches!(
        flow.input(key(KeyCode::Enter)),
        Progress::InputLimitExceeded
    ));
    assert!(flow.context().is_none());
    assert!(matches!(
        flow.input(key(KeyCode::Enter)),
        Progress::Inactive
    ));
    assert_eq!(called.get(), 0);
}
