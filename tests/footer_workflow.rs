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
                    Choice::new("second", 'b', ["Second", "Second", "B"]),
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
    flow.input(key(KeyCode::Left));
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
            key(KeyCode::Left),
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
fn defaults_repeats_modifiers_and_cancel_never_activate_disabled_choices() {
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
fn renderer_uses_one_row_cell_widths_and_only_complete_enabled_hits() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    use terminal_file_manager::{color::Theme, ui::workflow as presentation};
    for width in [0, 1, 2, 8, 24, 40, 80, 120, 160] {
        let flow = flow();
        let layout = presentation::layout(&flow, Rect::new(0, 2, width, 1));
        let mut terminal = Terminal::new(TestBackend::new(width, 4)).unwrap();
        terminal
            .draw(|frame| {
                presentation::render(
                    frame,
                    &layout,
                    &Theme::new(colored_text::ColorLevel::NoColor),
                )
            })
            .unwrap();
        for control in &layout.controls {
            assert_eq!(control.area.y, 2);
            assert_eq!(control.area.height, 1);
            assert!(control.area.right() <= width);
            assert_eq!(
                usize::from(control.area.width),
                ratatui::text::Line::raw(&control.label).width()
            );
            if !control.enabled {
                assert!(layout.hit(control.area.x, 2).is_none());
            }
        }
    }
}

#[test]
fn both_shift_tab_encodings_traverse_enabled_choices_and_unknown_clicks_are_inert() {
    let mut flow = flow();
    for code in [KeyCode::Tab, KeyCode::BackTab] {
        flow.input(Input::Key(KeyEvent::new(code, KeyModifiers::SHIFT)));
        assert_eq!(flow.highlighted().unwrap().id, "second");
        flow.input(key(KeyCode::Up));
        assert_eq!(flow.highlighted().unwrap().id, "first");
    }
    flow.input(Input::Click(
        terminal_file_manager::workflow::Target::Choice("missing"),
    ));
    assert_eq!(flow.stage_index(), 0);
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
    flow.input(key(KeyCode::Left));
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
                    vec![Choice::new("skip", 'a', ["Skip"; 3])],
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
