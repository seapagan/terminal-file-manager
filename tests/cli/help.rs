use super::*;

#[test]
fn help_describes_local_browsing_with_aligned_options_and_interactive_help() {
    for arguments in [&["--help"][..], &["-h"][..]] {
        let output = tfm(arguments);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).expect("help is UTF-8");
        assert!(text.contains("Usage: tfm"));
        assert_eq!(
            text.split_once("\n\n").unwrap().0,
            "Manage local files in one or two panes. The current directory is the default."
        );
        assert!(text.contains("? or F1 for interactive help"));
        for removed in [
            "unavailable",
            "Press r",
            "F5/F6",
            "F8 Trash",
            "Inside the browser:",
        ] {
            assert!(!text.contains(removed), "{text}");
        }
        let options = text.split_once("Options:\n").unwrap().1;
        for line in options.lines().take_while(|line| !line.is_empty()) {
            assert!(matches!(line[2..4].trim(), "" | "-2" | "-h" | "-V"));
            assert_eq!(&line[4..6], "  ");
            assert!(line[6..].starts_with("--"), "{line:?}");
            assert!(line[25..].starts_with(char::is_uppercase), "{line:?}");
        }
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn help_colours_semantic_elements_and_preserves_plain_text() {
    let plain = help_output(&[]);
    assert!(!plain.contains('\u{1b}'));
    assert_eq!(
        help_output(&[("NO_COLOR", "1"), ("FORCE_COLOR", "1")]),
        plain
    );
    let coloured = help_output(&[("FORCE_COLOR", "1")]);
    for heading in ["Usage:", "Options:"] {
        assert!(coloured.contains(&format!("\u{1b}[1;36m{heading}\u{1b}[0m")));
    }
    for flag in [
        "-2",
        "--two-panes",
        "--save-session",
        "--restore-session",
        "-h",
        "--help",
        "-V",
        "--version",
        "--",
    ] {
        assert!(
            coloured.contains(&format!("\u{1b}[32m{flag}\u{1b}[0m")),
            "{flag}: {coloured:?}"
        );
    }
    for placeholder in ["DIRECTORY", "OTHER_DIRECTORY"] {
        assert!(coloured.contains(&format!("\u{1b}[36m{placeholder}\u{1b}[0m")));
    }
    assert!(
        coloured
            .contains("press \u{1b}[36m?\u{1b}[0m or \u{1b}[36mF1\u{1b}[0m for interactive help.")
    );
    assert!(coloured.contains("\u{1b}[0m        Start with two panes\n"));
    let sgr = regex::Regex::new("\u{1b}\\[[0-9;]*m").unwrap();
    assert_eq!(sgr.replace_all(&coloured, ""), plain);
}

fn help_output(policy: &[(&str, &str)]) -> String {
    let root = tempfile::tempdir().unwrap();
    let mut command = isolated_session_command(root.path());
    for key in [
        "NO_COLOR",
        "FORCE_COLOR",
        "CLICOLOR_FORCE",
        "CLICOLOR",
        "TERM",
        "COLORTERM",
    ] {
        command.env_remove(key);
    }
    let output = command
        .arg("--help")
        .envs(policy.iter().copied())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}
