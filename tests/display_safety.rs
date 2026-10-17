//! Untrusted formatting characters are visible without changing native identities.
use terminal_file_manager::{fs, settings::validate_profile_name, ui};

const INVISIBLE: &[(char, &str)] = &[
    ('\u{200e}', "\\u{200e}"),
    ('\u{200f}', "\\u{200f}"),
    ('\u{61c}', "\\u{61c}"),
    ('\u{2028}', "\\u{2028}"),
    ('\u{2029}', "\\u{2029}"),
];

#[test]
fn direction_marks_and_separators_are_escaped_and_rejected_in_profile_names() {
    for (character, escaped) in INVISIBLE {
        let text = format!("界e\u{301}{character}name");
        assert_eq!(ui::clean(&text), format!("界e\u{301}{escaped}name"));
        assert!(validate_profile_name(&text).is_err());
    }
    let valid = "界e\u{301} café العربية";
    assert_eq!(ui::clean(valid), valid);
    assert!(validate_profile_name(valid).is_ok());
}

#[test]
fn display_escaping_preserves_native_names_paths_and_file_contents() {
    let root = tempfile::tempdir().unwrap();
    for (character, escaped) in INVISIBLE {
        let name = format!("界{character}file");
        let path = root.path().join(&name);
        std::fs::write(&path, b"native identity").unwrap();
        let entry = fs::inspect(&path);
        assert_eq!(entry.name, name.as_str());
        assert_eq!(entry.path, path);
        assert!(entry.identity.is_some() && entry.error.is_none());
        assert_eq!(
            ui::clean(entry.name.to_str().unwrap()),
            format!("界{escaped}file")
        );
        assert_eq!(std::fs::read(&entry.path).unwrap(), b"native identity");
    }
}
