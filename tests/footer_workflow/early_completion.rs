use super::key;
use crossterm::event::KeyCode;
use terminal_file_manager::workflow::{Choice, Flow, Input, Progress, Stage, Target};

#[derive(Debug, PartialEq)]
enum Outcome {
    Quick(usize),
    Normal(usize),
}

fn flow() -> Flow<usize, (Outcome, Vec<Input>)> {
    Flow::new(
        "dummy",
        "DUMMY",
        key(KeyCode::Char('x')),
        42,
        vec![
            Stage::new(
                "Action",
                vec![
                    Choice::new("quick", 'q', ["Quick"; 3]).completes(),
                    Choice::new("normal", 'n', ["Normal"; 3]),
                    Choice::new("disabled", 'd', ["Disabled"; 3])
                        .completes()
                        .disabled("Unavailable"),
                ],
                "quick",
            ),
            Stage::new(
                "Finish",
                vec![Choice::new("finish", 'f', ["Finish"; 3])],
                "finish",
            )
            .when(|_| true),
        ],
        |done| {
            let outcome = match done.choices.last().unwrap().id {
                "quick" => Outcome::Quick(done.context),
                "finish" => Outcome::Normal(done.context),
                _ => panic!("unexpected completion"),
            };
            (outcome, done.inputs)
        },
    )
    .unwrap()
}

#[test]
fn early_shortcut_default_and_mouse_return_the_distinct_callback_outcome_once() {
    for input in [
        key(KeyCode::Char('q')),
        key(KeyCode::Enter),
        Input::Click(Target::Choice("quick")),
    ] {
        let mut flow = flow();
        let Progress::Complete((outcome, inputs)) = flow.input(input.clone()) else {
            panic!("early completion")
        };
        assert_eq!(outcome, Outcome::Quick(42));
        assert_eq!(inputs, [key(KeyCode::Char('x')), input]);
        assert!(matches!(
            flow.input(key(KeyCode::Enter)),
            Progress::Inactive
        ));
    }
}

#[test]
fn disabled_early_choice_backtracking_and_normal_mouse_completion_keep_their_meaning() {
    let mut flow = flow();
    assert!(matches!(
        flow.input(key(KeyCode::Char('d'))),
        Progress::Disabled(_)
    ));
    assert_eq!(flow.stage_index(), 0);
    flow.input(key(KeyCode::Char('n')));
    assert_eq!(flow.stage_index(), 1);
    flow.input(key(KeyCode::Backspace));
    assert_eq!(flow.highlighted().unwrap().id, "normal");
    flow.input(key(KeyCode::Enter));
    let Progress::Complete((outcome, _)) = flow.input(Input::Click(Target::Choice("finish")))
    else {
        panic!("normal completion")
    };
    assert_eq!(outcome, Outcome::Normal(42));
    let mut cancelled = self::flow();
    assert!(matches!(
        cancelled.input(key(KeyCode::Esc)),
        Progress::Cancelled
    ));
    assert!(matches!(
        cancelled.input(key(KeyCode::Char('q'))),
        Progress::Inactive
    ));
}

#[test]
fn early_choice_bypasses_an_unusable_later_stage() {
    let mut flow = Flow::new(
        "dummy",
        "DUMMY",
        key(KeyCode::Char('x')),
        (),
        vec![
            Stage::new(
                "Action",
                vec![Choice::new("quick", 'q', ["Quick"; 3]).completes()],
                "quick",
            ),
            Stage::new(
                "Unavailable",
                vec![Choice::new("off", 'o', ["Off"; 3]).disabled("Unavailable")],
                "off",
            ),
        ],
        |done| done.choices.into_iter().map(|c| c.id).collect::<Vec<_>>(),
    )
    .unwrap();
    let Progress::Complete(choices) = flow.input(key(KeyCode::Enter)) else {
        panic!("early completion")
    };
    assert_eq!(choices, ["dummy", "quick"]);
}
