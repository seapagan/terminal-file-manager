//! Presentation only; the captured style evidence remains untouched.
use crate::color::{Role, Theme};
use ratatui::text::{Line, Span};

pub(super) fn lines(report: Vec<String>, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for text in report {
        let text = super::super::clean(&text);
        if is_heading(&text) {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.push(Line::styled(
                format!("── {text} ──"),
                theme.style(Role::Header),
            ));
        } else {
            lines.push(detail(&text, theme));
        }
    }
    lines
}

fn is_heading(text: &str) -> bool {
    matches!(
        text,
        "CAPABILITY" | "LAYOUT" | "STARTUP EVIDENCE" | "INTENDED STYLES" | "RENDERED CELLS"
    ) || text.starts_with("Pane 1 · ")
        || text.starts_with("Pane 2 · ")
}

fn detail(text: &str, theme: &Theme) -> Line<'static> {
    let Some((label, value)) = text.split_once(": ").or_else(|| text.split_once('=')) else {
        return Line::styled(text.to_owned(), theme.style(Role::Normal));
    };
    let label = label
        .trim_end()
        .replace(" intended", "")
        .replace(" rendered", "");
    let value = if value.starts_with("fg=") {
        value
            .replace("terminal default / unknown (Reset)", "default")
            .replace("modifiers=", "mods=")
            .replace("underline=", "ul=")
            .replace(" remove=NONE", "")
            .replace(" | ", "|")
    } else {
        value.to_owned()
    };
    Line::from(vec![
        Span::styled(format!("{label}: "), theme.style(Role::Label)),
        Span::styled(value, theme.style(Role::Value)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_preserves_path_and_environment_values() {
        let theme = Theme::new(colored_text::ColorLevel::Ansi16);
        for (text, expected) in [
            ("VTE_VERSION=<unset>", "VTE_VERSION: <unset>"),
            ("TERM=<unset>", "TERM: <unset>"),
            ("TERM=\"\"", "TERM: \"\""),
            ("VTE_VERSION=\"8400\"", "VTE_VERSION: \"8400\""),
            ("Cursor: /tmp/fg=one | two", "Cursor: /tmp/fg=one | two"),
            (
                "TERM=\"modifiers=one | two\"",
                "TERM: \"modifiers=one | two\"",
            ),
        ] {
            let line = detail(text, &theme);
            let shown: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            assert_eq!(shown, expected);
        }
    }
}
