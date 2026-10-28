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
    status: Option<String>,
    label: Option<String>,
    focused: bool,
    single_line: bool,
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
            status: None,
            label: None,
            focused: false,
            single_line: false,
        }
    }

    /// Informational label/value pair; it never creates an input target.
    pub fn key_value(label: &str, value: impl Into<String>) -> Self {
        let mut row = Self::plain(value);
        row.label = Some(if label.is_empty() {
            String::new()
        } else {
            format!("{}: ", super::super::clean(label))
        });
        row
    }

    /// A field whose focus and editing are owned by the caller.
    pub fn field(label: &str, value: &str, target: T, focused: bool) -> Self {
        Self {
            text: value.into(),
            role: Role::Value,
            field: Some(target),
            trailing: None,
            status: None,
            label: Some(format!("{label}: ")),
            focused,
            single_line: false,
        }
    }

    /// Keep a changing summary to one measured line.
    pub fn single_line(mut self) -> Self {
        self.single_line = true;
        self
    }

    /// Align a field value with a neighbouring informational label.
    pub fn label_width(mut self, cells: u16) -> Self {
        if let Some(label) = &mut self.label {
            label.push_str(&" ".repeat(usize::from(cells.saturating_sub(super::width(label)))));
        }
        self
    }

    /// Structured left filename/result label and indivisible right value.
    pub fn file(name: String, size: String) -> Self {
        Self {
            text: name,
            role: Role::Value,
            field: None,
            trailing: Some(size),
            status: None,
            label: None,
            focused: false,
            single_line: false,
        }
    }

    /// Reserve a short textual outcome before truncating the filename or size.
    pub fn file_status(mut self, status: &str) -> Self {
        self.status = Some(super::super::clean(status));
        self
    }

    /// Intrinsic width in escaped display cells, including the column gap.
    pub fn width(&self) -> usize {
        self.status
            .as_ref()
            .map_or(0, |status| usize::from(super::width(status)) + 1)
            + self
                .label
                .as_ref()
                .map_or(0, |label| Line::raw(label).width())
            + usize::from(self.focused)
            + Line::raw(super::super::clean(&self.text)).width()
            + self
                .trailing
                .as_ref()
                .map_or(0, |text| Line::raw(super::super::clean(text)).width() + 1)
    }

    /// Prose/fields wrap; structured rows remain one row with filename truncation.
    pub fn wrap(mut self, width: u16) -> Vec<Self> {
        self.text = super::super::clean(&self.text);
        if self.single_line {
            self.text = super::truncate(
                &self.text,
                width.saturating_sub(self.label.as_ref().map_or(0, |label| super::width(label))),
            );
            return vec![self];
        }
        if self.field.is_some() {
            if self
                .label
                .as_ref()
                .is_some_and(|label| super::width(label) + 2 > width)
            {
                let label = Self::styled(self.label.take().unwrap(), Role::Label);
                let mut rows = label.wrap(width);
                rows.push(self);
                return rows;
            }
            return vec![self];
        }
        if self.label.is_some() {
            return self.wrap_value(width);
        }
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
                status: None,
                label: None,
                focused: false,
                single_line: false,
            })
            .collect()
    }

    fn wrap_value(mut self, width: u16) -> Vec<Self> {
        let label = self.label.take().unwrap();
        let cells = super::width(&label);
        if cells >= width {
            let mut rows = Self::styled(label, Role::Label).wrap(width);
            rows.extend(self.wrap(width));
            return rows;
        }
        super::super::text::wrap_value(&self.text, usize::from(width - cells))
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let mut row = self.clone();
                row.text = text;
                row.label = Some(if index == 0 {
                    label.clone()
                } else {
                    " ".repeat(usize::from(cells))
                });
                row
            })
            .collect()
    }

    /// Render using the same column budget as measurement, with semantic size emphasis.
    pub fn line(&self, width: u16, theme: &Theme) -> Line<'static> {
        if self.field.is_some() {
            return self.field_line(width, theme);
        }
        if let Some(label) = &self.label {
            let style = if matches!(self.role, Role::Status) {
                theme.style(self.role).add_modifier(Modifier::BOLD)
            } else {
                theme.style(self.role)
            };
            return Line::from(vec![
                Span::styled(label.clone(), theme.style(Role::Label)),
                Span::styled(self.text.clone(), style),
            ]);
        }
        if let Some(status) = &self.status {
            let prefix = super::truncate(&format!("{status} "), width);
            let mut line = self.file_line(width.saturating_sub(super::width(&prefix)), theme);
            line.spans
                .insert(0, Span::styled(prefix, theme.style(self.role)));
            return line;
        }
        self.file_line(width, theme)
    }

    fn file_line(&self, width: u16, theme: &Theme) -> Line<'static> {
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

    fn field_line(&self, width: u16, theme: &Theme) -> Line<'static> {
        let label = self.label.clone().unwrap_or_default();
        let available = width.saturating_sub(super::width(&label));
        let caret = if self.focused && available > 0 {
            "▏"
        } else {
            ""
        };
        let budget = available.saturating_sub(super::width(caret));
        // Keep the editing end visible without changing the native field value.
        let mut excess = Line::raw(&self.text)
            .width()
            .saturating_sub(usize::from(budget));
        let mut start = 0;
        let line = Line::raw(&self.text);
        for grapheme in line.styled_graphemes(ratatui::style::Style::default()) {
            if excess == 0 {
                break;
            }
            start += grapheme.symbol.len();
            excess = excess.saturating_sub(usize::from(super::width(grapheme.symbol)));
        }
        let value = &self.text[start..];
        Line::from(vec![
            Span::styled(label, theme.style(Role::Label)),
            Span::styled(value.to_owned(), theme.style(Role::Value)),
            Span::styled(
                caret,
                theme.style(Role::Header).add_modifier(Modifier::BOLD),
            ),
        ])
    }
}
