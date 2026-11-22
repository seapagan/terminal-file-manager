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
