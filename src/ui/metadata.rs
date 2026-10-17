//! Shared label/value presentation for the metadata sidebar and floating popup.
use super::{
    clean,
    listing::{size, timestamp},
    text::wrap_value,
};
use crate::{
    app::{App, Detail, Pane},
    color::{Role, Theme},
    permissions::attributes,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

type Field = (&'static str, String);

pub(super) fn render(frame: &mut Frame, app: &App, bounds: Rect, theme: &Theme, floating: bool) {
    let fields = fields(&app.details[app.active], &app.panes[app.active]);
    let width = if floating {
        bounds.width.saturating_sub(4).min(90)
    } else {
        bounds.width
    };
    let lines = lines(&fields, width.saturating_sub(2), theme);
    let area = if floating {
        super::centered(
            bounds,
            width,
            u16::try_from(lines.len().saturating_add(2))
                .unwrap_or(u16::MAX)
                .min(bounds.height.saturating_sub(2)),
        )
    } else {
        bounds
    };
    let scroll = if floating {
        app.overlay_scroll
    } else {
        app.metadata_scroll
    };
    let max_scroll = lines
        .len()
        .saturating_sub(usize::from(area.height.saturating_sub(2)));
    let scroll = usize::from(scroll)
        .min(max_scroll)
        .min(usize::from(u16::MAX)) as u16;
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll, 0)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(if floating {
                    "Metadata — ↑/↓ or wheel"
                } else {
                    "Metadata — wheel scrolls"
                })
                .border_style(theme.style(Role::Header)),
        ),
        area,
    );
}

fn fields(detail: &Detail, pane: &Pane) -> Vec<Field> {
    let mut fields = entry_fields(detail, pane);
    if let Some(totals) = &detail.totals {
        match totals {
            Ok(totals) => {
                fields.push((
                    "Descendants",
                    format!(
                        "{} files, {} directories, {}{}",
                        totals.files,
                        totals.directories,
                        size(Some(totals.bytes)),
                        if totals.errors.is_empty() {
                            ""
                        } else {
                            " — INCOMPLETE"
                        }
                    ),
                ));
                fields.extend(totals.errors.iter().map(|error| ("Error", error.clone())));
            }
            Err(error) => fields.push(("Error", error.clone())),
        }
    }
    if detail.counting {
        fields.push(("Status", "Calculating totals… Esc cancels".into()));
    }
    fields.push(("Keys", "c: count/size on demand; Esc cancels/closes".into()));
    fields
}

fn entry_fields(detail: &Detail, pane: &Pane) -> Vec<Field> {
    let Some(entry) = &detail.entry else {
        return vec![(
            "Status",
            if detail.path.is_some() {
                "Loading metadata…"
            } else {
                "No visible entry"
            }
            .into(),
        )];
    };
    let mut fields = vec![
        ("Name", entry.name.to_string_lossy().into_owned()),
        ("Path", entry.path.display().to_string()),
        (
            "Canonical",
            detail
                .canonical
                .as_ref()
                .map_or_else(|| "unavailable".into(), |p| p.display().to_string()),
        ),
        ("Type", format!("{:?}", entry.kind)),
        ("Size", size(entry.size)),
        ("Modified UTC", timestamp(entry.modified)),
        ("Created UTC", timestamp(entry.created)),
        ("Accessed UTC", timestamp(entry.accessed)),
        ("Attributes", attributes(entry, pane.permissions)),
        ("Owner/group", entry.owner_display(pane.owner_display)),
        (
            "Link target",
            entry
                .link_target
                .as_ref()
                .map_or_else(|| "unavailable".into(), |p| p.display().to_string()),
        ),
    ];
    if let Some(error) = &entry.error {
        fields.push(("Error", error.clone()));
    }
    fields
}

fn lines(fields: &[Field], width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let label_width = label_width(fields, width);
    let value_width = usize::from(width).saturating_sub(label_width);
    fields
        .iter()
        .flat_map(|(label, value)| {
            let value_role = if *label == "Error" {
                Role::Error
            } else if *label == "Keys" {
                Role::Key
            } else {
                Role::Value
            };
            wrap_value(&clean(value), value_width)
                .into_iter()
                .enumerate()
                .map(move |(index, value)| {
                    let label = if index == 0 {
                        format!("{label}:")
                    } else {
                        String::new()
                    };
                    let label: String = label.chars().take(label_width.saturating_sub(1)).collect();
                    Line::from(vec![
                        Span::styled(format!("{label:label_width$}"), theme.style(Role::Label)),
                        Span::styled(value, theme.style(value_role)),
                    ])
                })
        })
        .collect()
}

fn label_width(fields: &[Field], width: u16) -> usize {
    fields
        .iter()
        .map(|(label, _)| label.len() + 2)
        .max()
        .unwrap_or(0)
        .min(usize::from(width / 2))
}

pub(super) fn scroll_limit(app: &App, bounds: Rect, floating: bool) -> u16 {
    let fields = fields(&app.details[app.active], &app.panes[app.active]);
    let width = if floating {
        bounds.width.saturating_sub(4).min(90)
    } else {
        bounds.width
    };
    let inner_width = width.saturating_sub(2);
    let value_width = usize::from(inner_width).saturating_sub(label_width(&fields, inner_width));
    let count: usize = fields
        .iter()
        .map(|(_, value)| wrap_value(&clean(value), value_width).len())
        .sum();
    let height = bounds.height.saturating_sub(if floating { 4 } else { 2 });
    u16::try_from(count.saturating_sub(usize::from(height))).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use colored_text::ColorLevel;

    #[test]
    fn wrapped_values_keep_graphemes_and_continue_under_value_column() {
        let theme = Theme::new(ColorLevel::Ansi16);
        let fields = vec![("Path", "界e\u{301}界e\u{301}界e\u{301}\u{1b}\n".into())];
        let lines = lines(&fields, 13, &theme);
        assert!(lines.len() > 1);
        let mut restored = String::new();
        for (index, line) in lines.iter().enumerate() {
            assert!(line.width() <= 13);
            assert_eq!(line.spans[0].width(), 6);
            assert_eq!(line.spans[0].content.trim().is_empty(), index != 0);
            assert!(!line.spans[1].content.contains('\u{1b}'));
            restored.push_str(&line.spans[1].content);
        }
        assert_eq!(restored, "界e\u{301}界e\u{301}界e\u{301}\\u{1b}\\n");
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        assert_ne!(lines[0].spans[0].style.fg, lines[0].spans[1].style.fg);
    }

    #[test]
    fn one_cell_values_do_not_overflow_or_split_combining_sequences() {
        assert_eq!(wrap_value("界e\u{301}", 1), ["�", "e\u{301}"]);
        assert_eq!(wrap_value("", 1), [""]);
        assert_eq!(wrap_value("value", 0), [""]);
    }
}
