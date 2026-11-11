use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use terminal_file_manager::workflow::{Choice, Flow, Input, Progress, Stage};

fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn flow() -> Flow<usize, (usize, Vec<Input>, Vec<&'static str>)> {
    Flow::new(
        "dummy",
        "DUMMY",
        key(KeyCode::Char('x')),
        42,
        vec![
            Stage::new(
                "Source",
                vec![
                    Choice::new("disabled", 'd', ["Disabled", "Off", "Off"])
                        .disabled("Unavailable"),
                    Choice::new("first", 'a', ["First", "First", "A"]),
                    Choice::new(
                        "second",
                        'b',
                        ["Second界e\u{301}", "Second界e\u{301}", "B界e\u{301}"],
                    ),
                ],
                "disabled",
            ),
            Stage::new(
                "Finish",
                vec![Choice::new("finish", 'f', ["Finish", "Finish", "F"])],
                "finish",
            ),
        ],
        |done| {
            (
                done.context,
                done.inputs,
                done.choices.into_iter().map(|c| c.id).collect(),
            )
        },
    )
    .unwrap()
}

#[test]
fn dummy_callback_gets_physical_detour_and_only_final_semantic_path_once() {
    let mut flow = flow();
    assert_eq!(flow.highlighted().unwrap().id, "first");
    assert!(matches!(
        flow.input(key(KeyCode::Char('d'))),
        Progress::Disabled(_)
    ));
    flow.input(key(KeyCode::Char('b')));
    flow.input(key(KeyCode::Backspace));
    assert_eq!(flow.highlighted().unwrap().id, "second");
    flow.input(key(KeyCode::Up));
    flow.input(Input::Click(
        terminal_file_manager::workflow::Target::Choice("first"),
    ));
    let Progress::Complete((context, inputs, resolved)) = flow.input(key(KeyCode::Enter)) else {
        panic!("completion")
    };
    assert_eq!(context, 42);
    assert_eq!(resolved, ["dummy", "first", "finish"]);
    assert_eq!(
        inputs,
        [
            key(KeyCode::Char('x')),
            key(KeyCode::Char('d')),
            key(KeyCode::Char('b')),
            key(KeyCode::Backspace),
            key(KeyCode::Up),
            Input::Click(terminal_file_manager::workflow::Target::Choice("first")),
            key(KeyCode::Enter)
        ]
    );
    assert!(matches!(
        flow.input(key(KeyCode::Enter)),
        Progress::Inactive
    ));
}

#[test]
fn repeats_and_modified_choices_preserve_default_until_enter_and_escape_cancels() {
    let mut flow = flow();
    let mut repeated = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    repeated.kind = KeyEventKind::Repeat;
    flow.input(Input::Key(repeated));
    flow.input(Input::Key(KeyEvent::new(
        KeyCode::Char('b'),
        KeyModifiers::ALT,
    )));
    assert_eq!(flow.stage_index(), 0);
    flow.input(key(KeyCode::Enter));
    assert_eq!(flow.stage_index(), 1);
    assert!(matches!(flow.input(key(KeyCode::Esc)), Progress::Cancelled));
    assert!(matches!(
        flow.input(key(KeyCode::Enter)),
        Progress::Inactive
    ));
}

#[test]
fn arrows_and_tab_are_consumed_without_changing_stage_or_default() {
    let mut flow = flow();
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Tab,
        KeyCode::BackTab,
    ] {
        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            assert!(matches!(
                flow.input(Input::Key(KeyEvent::new(code, modifiers))),
                Progress::Active
            ));
            assert_eq!(flow.highlighted().unwrap().id, "first");
            assert_eq!(flow.stage_index(), 0);
        }
    }
    flow.input(key(KeyCode::Enter));
    flow.input(key(KeyCode::Left));
    assert_eq!(flow.stage_index(), 1);
    flow.input(Input::Click(
        terminal_file_manager::workflow::Target::Choice("missing"),
    ));
    assert_eq!(flow.stage_index(), 1);
    flow.input(key(KeyCode::Backspace));
    assert!(matches!(
        flow.input(key(KeyCode::Backspace)),
        Progress::Cancelled
    ));
}

#[test]
fn lowercase_and_shift_letter_are_distinct_with_all_terminal_encodings() {
    for (letter, modifiers, expected) in [
        ('s', KeyModifiers::NONE, "this"),
        ('s', KeyModifiers::SHIFT, "other"),
        ('S', KeyModifiers::SHIFT, "other"),
        ('S', KeyModifiers::NONE, "other"),
    ] {
        let mut flow = Flow::new(
            "dummy",
            "DUMMY",
            key(KeyCode::Char('x')),
            (),
            vec![Stage::new(
                "Base",
                vec![
                    Choice::new("this", 's', ["This"; 3]),
                    Choice::new("other", 'S', ["Other"; 3]),
                ],
                "this",
            )],
            |done| done,
        )
        .unwrap();
        let input = Input::Key(KeyEvent::new(KeyCode::Char(letter), modifiers));
        let Progress::Complete(done) = flow.input(input.clone()) else {
            panic!("completion")
        };
        assert_eq!(done.choices[1].id, expected);
        assert_eq!(done.inputs.last(), Some(&input));
    }
}

#[test]
fn conditional_stages_skip_and_reenter_with_history_defaults_and_mouse_back() {
    use terminal_file_manager::workflow::Target;
    let mut flow = Flow::new(
        "dummy",
        "DUMMY",
        key(KeyCode::Char('x')),
        (),
        vec![
            Stage::new(
                "Branch",
                vec![
                    Choice::new("short", 'a', ["Short"; 3]),
                    Choice::new("long", 'b', ["Long"; 3]),
                ],
                "short",
            ),
            Stage::new(
                "Extra",
                vec![Choice::new("extra", 'e', ["Extra"; 3])],
                "extra",
            )
            .when(|choices| choices.last().is_some_and(|c| c.id == "long")),
            Stage::new("End", vec![Choice::new("end", 'f', ["End"; 3])], "end"),
        ],
        |done| done,
    )
    .unwrap();
    flow.input(key(KeyCode::Enter));
    assert_eq!(flow.stage().label, "End");
    flow.input(Input::Click(Target::Back));
    assert_eq!(flow.stage().label, "Branch");
    flow.input(Input::Click(Target::Choice("long")));
    assert_eq!(flow.stage().label, "Extra");
    flow.input(key(KeyCode::Enter));
    flow.input(key(KeyCode::Backspace));
    assert_eq!(flow.highlighted().unwrap().id, "extra");
    flow.input(key(KeyCode::Backspace));
    assert_eq!(flow.highlighted().unwrap().id, "long");
    flow.input(key(KeyCode::Char('a')));
    let Progress::Complete(done) = flow.input(Input::Click(Target::Choice("end"))) else {
        panic!("completion")
    };
    assert_eq!(
        done.choices.iter().map(|c| c.id).collect::<Vec<_>>(),
        ["dummy", "short", "end"]
    );
    assert!(done.inputs.contains(&Input::Click(Target::Choice("long"))));
}

#[test]
fn conditional_start_has_no_back_and_empty_available_definition_is_rejected() {
    use terminal_file_manager::workflow::Target;
    for available in [true, false] {
        let flow = Flow::new(
            "dummy",
            "DUMMY",
            key(KeyCode::Char('x')),
            (),
            vec![
                Stage::new(
                    "Skipped",
                    vec![Choice::new("skip", 'a', ["Skip"; 3]).disabled("Unavailable")],
                    "skip",
                )
                .when(|_| false),
                Stage::new(
                    "Start",
                    vec![Choice::new("start", 's', ["Start"; 3])],
                    "start",
                )
                .when(move |_| available),
            ],
            |done| done,
        );
        if available {
            let mut flow = flow.unwrap();
            assert_eq!(flow.stage().label, "Start");
            assert!(!flow.can_back());
            let layout = terminal_file_manager::ui::workflow::layout(
                &flow,
                ratatui::layout::Rect::new(0, 0, 120, 1),
            );
            assert!(
                !layout
                    .controls
                    .iter()
                    .any(|c| c.target == Some(Target::Back))
            );
            assert!(matches!(
                flow.input(Input::Click(Target::Cancel)),
                Progress::Cancelled
            ));
            assert!(matches!(
                flow.input(key(KeyCode::Enter)),
                Progress::Inactive
            ));
        } else {
            assert!(flow.is_err());
        }
    }
}

#[test]
fn no_enabled_start_is_rejected_even_when_conditionally_reachable() {
    for choices in [
        vec![],
        vec![Choice::new("off", 'o', ["Off"; 3]).disabled("Unavailable")],
    ] {
        let result = Flow::new(
            "dummy",
            "DUMMY",
            key(KeyCode::Char('x')),
            (),
            vec![Stage::new("Start", choices, "off").when(|_| true)],
            |done| done,
        );
        assert!(result.is_err());
    }
}

#[test]
fn unusable_conditional_stage_keeps_previous_choice_and_cannot_complete_a_partial_path() {
    use terminal_file_manager::workflow::Target;
    let mut required = Stage::new("Required", vec![Choice::new("off", 'o', ["Off"; 3])], "off");
    required.choices[0].disabled = Some("Unavailable".into());
    let mut flow = Flow::new(
        "dummy",
        "DUMMY",
        key(KeyCode::Char('x')),
        (),
        vec![
            Stage::new(
                "Branch",
                vec![
                    Choice::new("long", 'l', ["Long"; 3]),
                    Choice::new("short", 's', ["Short"; 3]),
                ],
                "long",
            ),
            required.when(|choices| choices.last().is_some_and(|c| c.id == "long")),
        ],
        |done| done,
    )
    .unwrap();
    assert!(
        matches!(flow.input(key(KeyCode::Enter)), Progress::Disabled(reason) if reason.contains("Required"))
    );
    assert_eq!(flow.stage_index(), 0);
    assert_eq!(flow.highlighted().unwrap().id, "long");
    assert!(!flow.can_back());
    assert!(matches!(
        flow.input(Input::Click(Target::Choice("long"))),
        Progress::Disabled(_)
    ));
    let Progress::Complete(done) = flow.input(Input::Click(Target::Choice("short"))) else {
        panic!("completion")
    };
    assert_eq!(
        done.choices.iter().map(|c| c.id).collect::<Vec<_>>(),
        ["dummy", "short"]
    );
    assert_eq!(done.inputs.len(), 4);
}

#[path = "footer_workflow/rendering.rs"]
mod rendering;

#[test]
fn activation_revalidates_consumer_availability_and_falls_back_to_an_enabled_default() {
    for enabled in [true, false] {
        let mut stage = Stage::new(
            "Start",
            vec![
                Choice::new("first", 'f', ["First"; 3]),
                Choice::new("second", 's', ["Second"; 3]),
            ],
            "first",
        );
        stage.choices[0].disabled = Some("Unavailable".into());
        if !enabled {
            stage.choices[1].disabled = Some("Unavailable".into());
        }
        let result = Flow::new(
            "dummy",
            "DUMMY",
            key(KeyCode::Char('x')),
            (),
            vec![stage],
            |done| done,
        );
        if enabled {
            assert_eq!(result.unwrap().highlighted().unwrap().id, "second");
        } else {
            assert!(result.is_err());
        }
    }
}

#[path = "footer_workflow/early_completion.rs"]
mod early_completion;

#[path = "footer_workflow/input_limit.rs"]
mod input_limit;

#[test]
fn malformed_choice_ids_and_shortcuts_are_rejected_before_activation() {
    for choices in [
        vec![Choice::new("first", '1', ["First"; 3])],
        vec![
            Choice::new("same", 'a', ["First"; 3]),
            Choice::new("same", 'b', ["Second"; 3]),
        ],
        vec![
            Choice::new("first", 'a', ["First"; 3]),
            Choice::new("second", 'a', ["Second"; 3]),
        ],
    ] {
        assert!(
            Flow::new(
                "dummy",
                "DUMMY",
                key(KeyCode::Char('x')),
                (),
                vec![Stage::new("Start", choices, "first")],
                |_| panic!("invalid workflow must never activate"),
            )
            .is_err()
        );
    }
}
