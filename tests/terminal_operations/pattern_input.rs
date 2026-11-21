//! Native pattern editing, rejection and control ownership.
use super::*;

#[test]
fn pattern_selection_alt_b_paste_mark_and_cancel_work_in_native_terminal() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("pattern-ready.txt"), "preserved").unwrap();
    let transcript = drive(
        command(root.path(), root.path(), None),
        &[
            ("pattern-ready.txt", "\x1bb"),
            ("Pattern selection", "\x1b[200~*\x1b[201~"),
            ("Matches: 1 / 1 selectable", "\r\r"),
            ("[*]", "\x1bb"),
            ("Pattern selection", "\x1bk\x1b[200~missing\x1b[201~\x1b"),
            ("Pattern selection cancelled", "q"),
        ],
        true,
    );
    assert!(transcript.contains("[*]"));
    assert_eq!(
        fs::read_to_string(root.path().join("pattern-ready.txt")).unwrap(),
        "preserved"
    );
}

#[test]
fn pattern_alt_controls_and_literal_letters_cross_the_native_terminal_parser() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("wxsmurQfile.txt"), "preserved").unwrap();
    let transcript = drive(
        command(root.path(), root.path(), None),
        &[
            ("wxsmurQfile.txt", "\x1bb"),
            ("Pattern selection", "\x1bs\x1bx\x1bwwxsmur?*.txt\x1bm"),
            ("[*]", "\x1bb"),
            ("Pattern selection", "\x1bx"),
            ("Regex searches", "\x1b"),
            ("Pattern selection cancelled", "\x1bb"),
            ("Pattern selection", "\x1bw\x1b[200~*\x1b[201~"),
            ("Matches: 1 / 1 selectable", "\x1bs\x1bw\x1bu"),
            ("0 visible entries", "q"),
        ],
        false,
    );
    assert!(transcript.contains("[*]"));
    assert_eq!(
        fs::read_to_string(root.path().join("wxsmurQfile.txt")).unwrap(),
        "preserved"
    );
}

#[test]
#[cfg(not(windows))]
fn pattern_overlong_native_input_requires_correction_before_apply() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("overlong-ready.txt"), "preserved").unwrap();
    // Keep each PTY write bounded; the final paste exceeds the field limit.
    let prefixes: [String; 5] =
        std::array::from_fn(|i| format!("\x1b[200~{}seed-{}\x1b[201~", "?".repeat(800), i + 1));
    let paste = format!("\x1b[200~{}\x1b[201~", "a".repeat(67));
    let steps = [
        ("overlong-ready.txt", "\x1bb"),
        ("Pattern selection", prefixes[0].as_str()),
        ("seed-1", prefixes[1].as_str()),
        ("seed-2", prefixes[2].as_str()),
        ("seed-3", prefixes[3].as_str()),
        ("seed-4", prefixes[4].as_str()),
        ("seed-5", paste.as_str()),
        // The warning suffix is new output; earlier letters can reuse old cells.
        ("Backspace to correct", "\x1bk\x1bm\x1bu\x1br\r\x1bx"),
        // The new Regex anchor proves blocked actions left the form open.
        ("^", "\x1bw\x7f"),
        ("Matches: 0 / 1 selectable", "\x1b"),
        ("Pattern selection cancelled", "\x1bb"),
        ("Pattern selection", "*\r\r"),
        ("[*]", "q"),
    ];
    let transcript = drive(command(root.path(), root.path(), None), &steps, false);
    assert!(transcript.contains("[*]"));
    assert_eq!(
        fs::read(root.path().join("overlong-ready.txt")).unwrap(),
        b"preserved"
    );
}
