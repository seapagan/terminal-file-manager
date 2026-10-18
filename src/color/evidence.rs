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
        let absent = if name == "VTE_VERSION" {
            "unset"
        } else {
            "<absent>"
        };
        lines.push(format!(
            "{name}={}",
            display_value(value.as_deref(), absent)
        ));
    }
    lines
}

fn display_value(value: Option<&std::ffi::OsStr>, absent: &str) -> String {
    let Some(value) = value else {
        return absent.into();
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
