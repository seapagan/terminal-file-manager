use super::*;

#[test]
fn technical_references_are_green_and_bold_in_each_colour_mode() {
    let mut c = controller(&Default::default());
    c.app.context_help.as_mut().unwrap().topic = HelpTopic::PathCopy;
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        let (buffer, body) = draw(&c.app, Rect::new(0, 0, 100, 180), level);
        for reference in [
            "wl-copy",
            "xclip",
            "xsel",
            "pbcopy",
            "OSC 52",
            "SSH",
            "Zellij",
            "tmux",
            "set-clipboard",
            "--save-session",
            "clipboard.osc52_fallback",
        ] {
            let (x, y) = find(&buffer, body, reference);
            assert_eq!(
                buffer[(x, y)].fg,
                theme.style(Role::Status).fg.unwrap_or_default(),
                "{reference}"
            );
            assert!(
                buffer[(x, y)].modifier.contains(Modifier::BOLD),
                "{reference}"
            );
        }
        let (x, y) = find(&buffer, body, "Enter");
        assert_eq!(
            buffer[(x, y)].fg,
            theme.style(Role::Key).fg.unwrap_or_default()
        );
        let (x, y) = find(&buffer, body, "commands");
        assert!(!buffer[(x, y)].modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn main_help_descriptions_use_the_shared_reference_and_key_styles() {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::TrueColor,
    ] {
        let mut c = controller(&std::collections::BTreeMap::from([(
            "Alt+界".into(),
            Action::Zoxide,
        )]));
        c.app.context_help = None;
        c.app.help = true;
        let mut terminal = Terminal::new(TestBackend::new(100, 180)).unwrap();
        let mut geometry = None;
        let theme = Theme::new(level);
        terminal
            .draw(|f| geometry = Some(ui::render(f, &c.app, &theme)))
            .unwrap();
        let body = geometry.unwrap().help.unwrap().body;
        let buffer = terminal.backend().buffer();
        for (text, role) in [
            ("zoxide", Role::HelpReference),
            ("Alt+界", Role::Key),
            ("Enter", Role::Key),
        ] {
            let (x, y) = find(buffer, body, text);
            assert_eq!(buffer[(x, y)].fg, theme.style(role).fg.unwrap_or_default());
            assert!(buffer[(x, y)].modifier.contains(Modifier::BOLD));
        }
    }
}

#[test]
fn search_results_keep_reference_styles_and_the_selected_result_style() {
    use terminal_file_manager::input::HelpPicker;
    let mut c = controller(&Default::default());
    c.app.context_help = None;
    c.app.help = true;
    c.app.help_picker = Some(HelpPicker::default());
    c.app.help_picker.as_mut().unwrap().query = "zoxide".into();
    let theme = Theme::new(ColorLevel::TrueColor);
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    let mut geometry = None;
    terminal
        .draw(|f| geometry = Some(ui::render(f, &c.app, &theme)))
        .unwrap();
    let picker = geometry.unwrap().help.unwrap().picker.unwrap();
    assert_eq!(picker.controls.len(), 2);
    let buffer = terminal.backend().buffer();
    for (index, (area, _)) in picker.controls.iter().enumerate() {
        let (x, y) = find(buffer, *area, "zoxide");
        let role = if index == 0 {
            Role::Cursor
        } else {
            Role::HelpReference
        };
        assert_eq!(buffer[(x, y)].fg, theme.style(role).fg.unwrap_or_default());
        assert!(buffer[(x, y)].modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn copy_path_labels_are_quoted_without_key_or_reference_styling() {
    let mut c = controller(&Default::default());
    c.app.context_help.as_mut().unwrap().topic = HelpTopic::PathCopy;
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        let (buffer, body) = draw(&c.app, Rect::new(0, 0, 100, 180), level);
        for label in [
            "From",
            "Format",
            "Set base",
            "Other base",
            "Marked",
            "Highlighted",
            "Current folder",
            "[×]",
            "Name",
            "Relative",
            "Full path",
        ] {
            let (x, y) = find(&buffer, body, &format!("‘{label}’"));
            assert_eq!(
                buffer[(x, y)].fg,
                Theme::new(level).style(Role::Value).fg.unwrap_or_default()
            );
            assert!(!buffer[(x, y)].modifier.contains(Modifier::BOLD));
        }
        let prose: String = (body.y..body.bottom())
            .map(|y| row(&buffer, body, y))
            .collect();
        assert!(prose.contains("relative paths"));
        assert!(!prose.contains("‘relative paths’"));
    }
}

use terminal_file_manager::input::ViewHelpTopic;

const LABEL_CASES: &[(HelpTopic, &[&str])] = &[
    (
        HelpTopic::Pattern,
        &[
            "Pattern",
            "Wildcard",
            "Regex",
            "Case sensitive",
            "Mark",
            "Unmark",
            "Replace",
        ],
    ),
    (
        HelpTopic::Conflict,
        &[
            "Incoming",
            "Existing",
            "Skip",
            "Replace",
            "Keep both",
            "Newer",
            "Apply to all",
        ],
    ),
    (
        HelpTopic::Jobs,
        &["Evidence", "Cancel", "Retry", "Reconcile"],
    ),
    (
        HelpTopic::Exit,
        &["Return", "Wait then exit", "Cancel safely then exit"],
    ),
    (
        HelpTopic::View(ViewHelpTopic::Profiles),
        &["Apply selected to both", "Save active as…"],
    ),
    (HelpTopic::View(ViewHelpTopic::Options), &["Modified"]),
    (
        HelpTopic::Policy(Action::Sort),
        &[
            "Natural",
            "Name",
            "Extension",
            "Size",
            "Modified",
            "Created",
            "Kind",
            "Version",
            "Unsorted",
            "Reverse",
            "[?]",
        ],
    ),
    (
        HelpTopic::Policy(Action::Group),
        &["First", "Last", "Mixed"],
    ),
    (HelpTopic::Policy(Action::Hidden), &["Hide", "Show"]),
    (HelpTopic::Policy(Action::Ignore), &["Dim", "Hide", "Off"]),
];

#[test]
fn contextual_controls_use_their_actual_quoted_labels() {
    for &(topic, labels) in LABEL_CASES {
        let mut c = controller(&Default::default());
        c.app.context_help.as_mut().unwrap().topic = topic;
        let (buffer, body) = draw(&c.app, Rect::new(0, 0, 100, 180), ColorLevel::TrueColor);
        for label in labels {
            let (x, y) = find(&buffer, body, &format!("‘{label}’"));
            assert!(
                !buffer[(x, y)].modifier.contains(Modifier::BOLD),
                "{topic:?} {label}"
            );
        }
    }
}
