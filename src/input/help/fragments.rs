//! Shared semantic content for Help prose, command descriptions and CLI output.
use crate::color::Role;

#[derive(Clone, Debug)]
pub enum HelpFragment {
    Text(String),
    Key(String),
    Reference(String),
    Label(String),
}

pub type HelpParagraph = Vec<HelpFragment>;

impl HelpFragment {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(text) | Self::Key(text) | Self::Reference(text) | Self::Label(text) => text,
        }
    }

    pub(crate) fn role(&self) -> Role {
        match self {
            Self::Key(_) => Role::Key,
            Self::Reference(_) => Role::HelpReference,
            Self::Text(_) | Self::Label(_) => Role::Value,
        }
    }

    pub(crate) fn stdout(&self) -> String {
        use colored_text::{Colorize, RenderTarget};
        match self {
            Self::Key(text) => text.cyan().bold().render(RenderTarget::Stdout),
            Self::Reference(text) => text.green().bold().render(RenderTarget::Stdout),
            Self::Text(text) | Self::Label(text) => text.clone(),
        }
    }
}

// Backticks mark fixed keys; {} inserts an opaque effective binding.
pub(crate) fn paragraph(text: &str, bindings: &[&str]) -> HelpParagraph {
    let mut bindings = bindings.iter();
    let mut fragments = Vec::new();
    for (index, part) in text
        .split('`')
        .enumerate()
        .filter(|(_, part)| !part.is_empty())
    {
        if index % 2 == 0 {
            fragments.extend(prose(part));
        } else {
            let key = if part == "{}" {
                *bindings.next().expect("shortcut placeholder")
            } else {
                part
            };
            fragments.push(if key == "Unbound" {
                HelpFragment::Text("Not bound".into())
            } else {
                HelpFragment::Key(key.into())
            });
        }
    }
    fragments
}

fn prose(text: &str) -> HelpParagraph {
    static TOKENS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let tokens = TOKENS.get_or_init(|| regex::Regex::new(
        r"‘[^’]+’|\b(?:clipboard\.osc52_fallback = true|set-clipboard on|OSC 52|GNU screen|wl-copy|xclip|xsel|pbcopy|zoxide|Zellij|tmux|SSH|Wayland|X11|SHA-256|Enter|Esc)\b|--[a-z][a-z-]*"
    ).expect("Help semantic tokens"));
    let mut fragments = Vec::new();
    let mut end = 0;
    for token in tokens.find_iter(text) {
        if token.start() > end {
            fragments.push(HelpFragment::Text(text[end..token.start()].into()));
        }
        let value = token.as_str().to_owned();
        fragments.push(match token.as_str() {
            "Enter" | "Esc" => HelpFragment::Key(value),
            text if text.starts_with('‘') => HelpFragment::Label(value),
            _ => HelpFragment::Reference(value),
        });
        end = token.end();
    }
    if end < text.len() {
        fragments.push(HelpFragment::Text(text[end..].into()));
    }
    fragments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_keys_references_and_ordinary_words_stay_distinct() {
        let parts = paragraph(
            "‘Name’ names `Enter` `{}` tmux tmuxes ‘OSC 52’",
            &["Alt+` / Alt+界"],
        );
        assert!(matches!(&parts[0], HelpFragment::Label(text) if text == "‘Name’"));
        assert!(
            parts
                .iter()
                .any(|p| matches!(p, HelpFragment::Key(text) if text == "Alt+` / Alt+界"))
        );
        assert_eq!(
            parts
                .iter()
                .filter(|p| matches!(p, HelpFragment::Reference(_)))
                .count(),
            1
        );
        assert!(matches!(parts.last().unwrap(), HelpFragment::Label(text) if text == "‘OSC 52’"));
        assert_eq!(
            parts.iter().map(HelpFragment::text).collect::<String>(),
            "‘Name’ names Enter Alt+` / Alt+界 tmux tmuxes ‘OSC 52’"
        );
    }
}
