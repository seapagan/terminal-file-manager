//! Evidence only: capability policy and precedence remain owned by colored_text.
use colored_text::ColorizeConfig;
use std::io::IsTerminal;

pub(super) fn startup(vte_version: Option<&std::ffi::OsStr>) -> Vec<String> {
    let mut lines = vec![
        format!("Startup stdout TTY: {}", std::io::stdout().is_terminal()),
        format!(
            "colored_text mode={:?}, depth={:?}",
            ColorizeConfig::color_mode(),
            ColorizeConfig::color_depth_mode()
        ),
        "Detection reason API unavailable; environment evidence follows:".into(),
    ];
    for name in [
        "NO_COLOR",
        "FORCE_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
        "COLORTERM",
        "VTE_VERSION",
        "WT_SESSION",
        "ConEmuANSI",
        "ANSICON",
        "CI",
    ] {
        let value = if name == "VTE_VERSION" {
            vte_version.map(std::ffi::OsStr::to_owned)
        } else {
            std::env::var_os(name)
        };
        lines.push(format!("{name}={}", display_value(value.as_deref())));
    }
    lines
}

fn display_value(value: Option<&std::ffi::OsStr>) -> String {
    let Some(value) = value else {
        return "<unset>".into();
    };
    let text = format!("{value:?}");
    if text.chars().count() > 256 {
        format!(
            "{}… [truncated]",
            text.chars().take(256).collect::<String>()
        )
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn captured_environment_values_distinguish_unset_empty_and_text() {
        for (value, expected) in [
            (None, "VTE_VERSION=<unset>"),
            (Some(OsStr::new("")), "VTE_VERSION=\"\""),
            (Some(OsStr::new("8400")), "VTE_VERSION=\"8400\""),
        ] {
            assert!(startup(value).iter().any(|line| line == expected));
        }
    }

    #[test]
    fn environment_formatter_preserves_empty_and_quoted_term_values() {
        for (value, expected) in [
            (None, "<unset>"),
            (Some(OsStr::new("")), "\"\""),
            (Some(OsStr::new("xterm-256color")), "\"xterm-256color\""),
        ] {
            assert_eq!(display_value(value), expected);
        }
    }
}
