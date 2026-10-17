//! Evidence only: capability policy and precedence remain owned by colored_text.
use colored_text::ColorizeConfig;
use std::io::IsTerminal;

pub(super) fn startup() -> Vec<String> {
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
        "WT_SESSION",
        "ConEmuANSI",
        "ANSICON",
        "CI",
    ] {
        let value = std::env::var_os(name).map_or_else(
            || "<absent>".into(),
            |value| {
                let text = format!("{value:?}");
                if text.chars().count() > 256 {
                    format!(
                        "{}… [truncated]",
                        text.chars().take(256).collect::<String>()
                    )
                } else {
                    text
                }
            },
        );
        lines.push(format!("{name}={value}"));
    }
    lines
}
