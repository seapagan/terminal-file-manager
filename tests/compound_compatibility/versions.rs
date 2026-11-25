use super::*;

#[test]
fn semver_precedence_controls_missing_older_equal_and_newer_checks() {
    for (recorded, running, due) in [
        (None, "0.1.0", true),
        (Some("0.0.9"), "0.1.0", true),
        (Some("0.1.0"), "0.1.0", false),
        (Some("0.2.0"), "0.1.0", false),
        (Some("0.1.0-pre"), "0.1.0", true),
        (Some("0.1.0-beta0"), "0.1.0-beta.1", false),
        (Some("0.1.0-beta.1"), "0.1.0-beta0", true),
        (Some("0.1.0-beta.1"), "0.1.0-beta.2", true),
        (Some("0.1.0-pre"), "0.1.0-rc.2", true),
        (Some("0.1.0-rc.2"), "0.1.0-rc.10", true),
        (Some("0.1.0-rc.2"), "0.1.0", true),
        (Some("0.1.0"), "0.1.0-pre", false),
        (Some("0.1.0+build.2"), "0.1.0+build.9", false),
    ] {
        let settings = Settings {
            compound_extensions_checked_version: recorded.map(str::to_owned),
            ..Default::default()
        };
        assert_eq!(
            settings.compound_check_due(running).unwrap(),
            due,
            "{recorded:?} -> {running}"
        );
    }
}

#[test]
fn no_difference_checks_complete_without_a_popup_and_preserve_configured_lists() {
    let defaults = Settings::default().effective_compound_extensions();
    let mut reordered = defaults.clone();
    reordered.reverse();
    reordered.push(defaults[0].clone());
    for replacement in [None, Some(defaults), Some(reordered)] {
        let settings = Settings {
            compound_extensions: replacement.clone(),
            compound_extensions_extend: vec![".user.js".into()],
            ..Default::default()
        };
        let text = toml::to_string(&settings).unwrap();
        let c = controller(&text, "0.1.0");
        assert!(c.app.compound_extension_review.is_none());
        assert_eq!(c.preferences.compound_extensions, replacement);
        assert_eq!(c.preferences.compound_extensions_extend, [".user.js"]);
        assert_eq!(
            c.preferences.compound_extensions_checked_version.as_deref(),
            Some("0.1.0")
        );
    }
}

#[test]
fn skipped_checks_preserve_recorded_versions_and_configured_lists() {
    for marker in ["0.1.0", "0.1.1", "0.1.0+build.2"] {
        let c = controller(
            &format!(
                "compound_extensions = []\ncompound_extensions_extend = ['.tar.gz']\ncompound_extensions_checked_version = '{marker}'"
            ),
            "0.1.0",
        );
        assert!(c.app.compound_extension_review.is_none());
        assert!(!c.preferences_changed);
        assert_eq!(
            c.preferences.compound_extensions_checked_version.as_deref(),
            Some(marker)
        );
        assert_eq!(c.preferences.compound_extensions, Some(vec![]));
        assert_eq!(c.preferences.compound_extensions_extend, [".tar.gz"]);
    }
}
