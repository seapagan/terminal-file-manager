use crate::color::{Role, Theme};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};

/// A prose/value, editable field, warning, separator or structured file/result row.
/// Native operands remain in the caller; these strings are display-only.
#[derive(Clone, Debug)]
pub struct Row<T> {
    /// Text is escaped before measuring, wrapping or rendering.
    pub text: String,
    /// Semantic text style.
    pub role: Role,
    /// Optional typed field focus target.
    pub field: Option<T>,
    /// An indivisible right-aligned value (for example a file size).
    pub trailing: Option<String>,
}

impl<T: Clone> Row<T> {
    /// Ordinary explanatory text or labelled value.
    pub fn plain(text: impl Into<String>) -> Self {
        Self::styled(text, Role::Value)
    }

    /// Warning/error or other semantic content. Empty text separates sections.
    pub fn styled(text: impl Into<String>, role: Role) -> Self {
        Self {
            text: text.into(),
            role,
            field: None,
            trailing: None,
        }
    }

    /// A field whose focus and editing are owned by the caller.
    pub fn field(label: &str, value: &str, target: T, focused: bool) -> Self {
        Self {
            text: format!("{}{label}: {value}", if focused { "> " } else { "  " }),
            role: if focused { Role::Cursor } else { Role::Value },
            field: Some(target),
            trailing: None,
        }
    }

    /// Structured left filename/result label and indivisible right value.
    pub fn file(name: String, size: String) -> Self {
        Self {
            text: name,
            role: Role::Value,
            field: None,
            trailing: Some(size),
        }
    }

    /// Intrinsic width in escaped display cells, including the column gap.
    pub fn width(&self) -> usize {
        Line::raw(super::super::clean(&self.text)).width()
            + self
                .trailing
                .as_ref()
                .map_or(0, |text| Line::raw(super::super::clean(text)).width() + 2)
    }

    /// Prose/fields wrap; structured rows remain one row with filename truncation.
    pub fn wrap(mut self, width: u16) -> Vec<Self> {
        self.text = super::super::clean(&self.text);
        if let Some(trailing) = &mut self.trailing {
            *trailing = super::super::clean(trailing);
            return vec![self];
        }
        super::super::text::wrap_value(&self.text, usize::from(width))
            .into_iter()
            .map(|text| Self {
                text,
                role: self.role,
                field: self.field.clone(),
                trailing: None,
            })
            .collect()
    }

    /// Render using the same column budget as measurement, with semantic size emphasis.
    pub fn line(&self, width: u16, theme: &Theme) -> Line<'static> {
        let Some(value) = &self.trailing else {
            let mut style = theme.style(self.role);
            if matches!(self.role, Role::Status) {
                style = style.add_modifier(Modifier::BOLD);
            }
            return Line::styled(self.text.clone(), style);
        };
        let text = super::file_row(&self.text, value, width);
        if !text.ends_with(value) {
            return Line::styled(text, theme.style(self.role));
        }
        let prefix = text[..text.len() - value.len()].to_owned();
        Line::from(vec![
            Span::styled(prefix, theme.style(self.role)),
            Span::styled(
                value.clone(),
                theme.style(Role::Status).add_modifier(Modifier::BOLD),
            ),
        ])
    }
}
