//! Compound suffix preferences preserve replacement and additive configuration.
use terminal_file_manager::settings::{Settings, Store};

const DEFAULTS: [&str; 7] = [
    ".tar.gz",
    ".tar.xz",
    ".tar.bz2",
    ".tar.zst",
    ".tar.lz",
    ".tar.lzma",
    ".tar.Z",
];

#[test]
fn replacement_and_additions_resolve_in_order_with_explicit_empty_lists() {
    for (text, expected) in [
        ("", DEFAULTS.to_vec()),
        ("compound_extensions_extend = []", DEFAULTS.to_vec()),
        (
            "compound_extensions_extend = ['.tar.br', '.user.js']",
            [DEFAULTS.as_slice(), &[".tar.br", ".user.js"]].concat(),
        ),
        (
            "compound_extensions = ['.tar.gz', '.tar.xz']",
            vec![".tar.gz", ".tar.xz"],
        ),
        (
            "compound_extensions = ['.tar.gz', '.tar.xz']\ncompound_extensions_extend = ['.tar.br', '.user.js']",
            vec![".tar.gz", ".tar.xz", ".tar.br", ".user.js"],
        ),
        ("compound_extensions = []", vec![]),
        (
            "compound_extensions = []\ncompound_extensions_extend = []",
            vec![],
        ),
        (
            "compound_extensions = []\ncompound_extensions_extend = ['.user.js']",
            vec![".user.js"],
        ),
        (
            "compound_extensions = ['.user.js']\ncompound_extensions_extend = []",
            vec![".user.js"],
        ),
    ] {
        let settings = Settings::parse(text).unwrap();
        assert_eq!(settings.effective_compound_extensions(), expected, "{text}");
    }
}

#[test]
fn exact_duplicates_keep_the_first_occurrence_without_folding_case() {
    for (text, expected) in [
        (
            "compound_extensions_extend = ['.tar.gz', '.user.js', '.tar.Z', '.user.js', '.tar.z']",
            [DEFAULTS.as_slice(), &[".user.js", ".tar.z"]].concat(),
        ),
        (
            "compound_extensions = ['.user.js', '.tar.gz', '.user.js']\ncompound_extensions_extend = ['.tar.gz', '.USER.js', '.tar.br', '.tar.br']",
            vec![".user.js", ".tar.gz", ".USER.js", ".tar.br"],
        ),
    ] {
        assert_eq!(
            Settings::parse(text)
                .unwrap()
                .effective_compound_extensions(),
            expected
        );
    }
}

#[test]
fn both_lists_validate_suffixes_and_report_the_offending_setting() {
    for name in ["compound_extensions", "compound_extensions_extend"] {
        for suffix in [
            "",
            "rs",
            ".",
            ".a.",
            ".a/rs",
            ".a\\rs",
            ".a\nrs",
            ".a\u{7f}rs",
        ] {
            let value = toml::Value::Array(vec![toml::Value::String(suffix.into())]);
            let text = format!("{name} = {value}");
            assert_eq!(
                Settings::parse(&text).unwrap_err(),
                format!("{name} must contain dotted filename suffixes without path separators")
            );
        }
    }
    let error =
        Settings::parse("compound_extensions = ['.tar.gz']\ncompound_extensions_extend = ['bad']")
            .unwrap_err();
    assert!(error.starts_with("compound_extensions_extend "));
}

#[test]
fn serialization_and_save_reload_preserve_each_configured_list() {
    for text in [
        "",
        "compound_extensions_extend = []",
        "compound_extensions_extend = ['.tar.br', '.tar.br']",
        "compound_extensions = []",
        "compound_extensions = []\ncompound_extensions_extend = ['.user.js']",
        "compound_extensions = ['.tar.gz', '.tar.gz']",
        "compound_extensions = ['.tar.xz']\ncompound_extensions_extend = ['.user.js', '.tar.xz']",
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        let mut store = Store::load(path.clone()).unwrap();
        let replacement = store.settings.compound_extensions.clone();
        let additions = store.settings.compound_extensions_extend.clone();
        let effective = store.settings.effective_compound_extensions();
        store.settings.sidebar = true;
        let serialized = toml::to_string_pretty(&store.settings).unwrap();
        let table: toml::Table = toml::from_str(&serialized).unwrap();
        assert_eq!(
            table.contains_key("compound_extensions"),
            replacement.is_some()
        );
        let parsed = Settings::parse(&serialized).unwrap();
        assert_eq!(parsed.compound_extensions, replacement);
        assert_eq!(parsed.compound_extensions_extend, additions);
        store.save().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), serialized);
        let reloaded = Store::load(path).unwrap().settings;
        assert!(reloaded.sidebar);
        assert_eq!(reloaded.compound_extensions, replacement);
        assert_eq!(reloaded.compound_extensions_extend, additions);
        assert_eq!(reloaded.effective_compound_extensions(), effective);
    }
}

#[test]
fn saving_a_missing_config_keeps_the_replacement_omitted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let store = Store::load(path.clone()).unwrap();
    store.save().unwrap();
    let reloaded = Store::load(path).unwrap().settings;
    assert!(reloaded.compound_extensions.is_none());
    assert!(reloaded.compound_extensions_extend.is_empty());
    assert_eq!(reloaded.effective_compound_extensions(), DEFAULTS);
}

#[test]
fn checked_versions_accept_semver_and_diagnose_invalid_markers() {
    for version in [
        "0.1.0",
        "0.1.0-pre",
        "0.1.0-beta0",
        "0.1.0-beta.1",
        "0.1.0-rc.2",
    ] {
        let text = format!("compound_extensions_checked_version = '{version}'");
        assert!(Settings::parse(&text).is_ok(), "{version}");
    }
    for version in ["", "0.1", "v0.1.0", "0.1.0-01", "nonsense"] {
        let error = Settings::parse(&format!(
            "compound_extensions_checked_version = '{version}'"
        ))
        .unwrap_err();
        assert!(error.contains("compound_extensions_checked_version"));
    }
}
