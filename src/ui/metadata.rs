//! Shared label/value presentation for the metadata sidebar and floating popup.
use super::{
    clean,
    listing::{listing_timestamp, permission_spans, size},
    text::wrap_line,
};
use crate::{
    app::{App, Detail, MetadataFormat},
    color::{Role, Theme},
    input::Action,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

struct Field {
    label: &'static str,
    value: Line<'static>,
    action: Option<Action>,
}

impl Field {
    fn plain(label: &'static str, value: String, theme: &Theme) -> Self {
        let role = match label {
            "Error" => Role::Error,
            "Keys" => Role::Key,
            _ => Role::Value,
        };
        Self {
            label,
            value: Line::styled(clean(&value), theme.style(role)),
            action: None,
        }
    }
    fn interactive(mut self, detail: &Detail, format: &MetadataFormat, theme: &Theme) -> Self {
        if self.label == "Attributes"
            && let Some(entry) = &detail.entry
        {
            self.value = Line::from(permission_spans(entry, format.permissions, theme));
            self.action = Some(Action::Permissions);
        } else if self.label == "Owner/group" {
            self.action = Some(Action::OwnerDisplay);
        } else if time_label(self.label) {
            self.action = Some(Action::FuzzyTimestamps);
        }
        self
    }
}

/// A visible value run; labels, borders and padding are never actionable.
#[derive(Clone, Debug)]
pub struct MetadataControl {
    pub area: Rect,
    pub action: Action,
}

type Row = (Line<'static>, Option<Action>, u16);

struct Layout {
    area: Rect,
    lines: Vec<Line<'static>>,
    controls: Vec<MetadataControl>,
    scroll: u16,
    limit: u16,
}

pub(super) fn render(frame: &mut Frame, app: &App, bounds: Rect, theme: &Theme, floating: bool) {
    let layout = layout(app, bounds, theme, floating);
    let area = layout.area;
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(layout.lines)
            .scroll((layout.scroll, 0))
            .block(
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

fn fields(
    detail: &Detail,
    pane: &MetadataFormat,
    now: std::time::SystemTime,
    theme: &Theme,
) -> Vec<Field> {
    let mut fields = entry_fields(detail, pane, now);
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
        .into_iter()
        .map(|(label, value)| Field::plain(label, value, theme).interactive(detail, pane, theme))
        .collect()
}

fn time_label(label: &str) -> bool {
    matches!(label, "Modified" | "Created" | "Accessed")
}

fn time_field(
    label: &'static str,
    time: Option<std::time::SystemTime>,
    pane: &MetadataFormat,
    now: std::time::SystemTime,
) -> (&'static str, String) {
    let mut value = listing_timestamp(time, pane.fuzzy_timestamps, now);
    if !pane.fuzzy_timestamps && time.is_some() {
        value.push_str(" (UTC)");
    }
    (label, value)
}

fn entry_fields(
    detail: &Detail,
    pane: &MetadataFormat,
    now: std::time::SystemTime,
) -> Vec<(&'static str, String)> {
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
        time_field("Modified", entry.modified, pane, now),
        time_field("Created", entry.created, pane, now),
        time_field("Accessed", entry.accessed, pane, now),
        (
            "Attributes",
            crate::permissions::attributes(entry, pane.permissions),
        ),
        ("Owner/group", entry.owner_display(pane.owner_display)),
    ];
    fields.extend(link_field(entry));
    if let Some(error) = &entry.error {
        fields.push(("Error", error.clone()));
    }
    fields
}

fn link_field(entry: &crate::fs::Entry) -> Option<(&'static str, String)> {
    (matches!(
        entry.kind,
        crate::fs::EntryKind::Link
            | crate::fs::EntryKind::Junction
            | crate::fs::EntryKind::ReparsePoint
    ) || entry.link_target.is_some())
    .then(|| {
        (
            "Link target",
            entry
                .link_target
                .as_ref()
                .map_or_else(|| "unavailable".into(), |p| p.display().to_string()),
        )
    })
}

fn rows(fields: &[Field], width: u16, theme: &Theme) -> Vec<Row> {
    let label_width = label_width(fields, width);
    let value_width = usize::from(width).saturating_sub(label_width);
    fields
        .iter()
        .flat_map(|field| {
            wrap_line(field.value.clone(), value_width)
                .into_iter()
                .enumerate()
                .map(move |(index, value)| {
                    let label = if index == 0 {
                        format!("{}:", field.label)
                    } else {
                        String::new()
                    };
                    let label: String = label.chars().take(label_width.saturating_sub(1)).collect();
                    let length = value.width() as u16;
                    let mut spans = vec![Span::styled(
                        format!("{label:label_width$}"),
                        theme.style(Role::Label),
                    )];
                    spans.extend(value.spans);
                    (Line::from(spans), field.action, length)
                })
        })
        .collect()
}

fn label_width(fields: &[Field], width: u16) -> usize {
    fields
        .iter()
        .map(|field| field.label.len() + 2)
        .max()
        .unwrap_or(0)
        .min(usize::from(width / 2))
}

fn layout(app: &App, bounds: Rect, theme: &Theme, floating: bool) -> Layout {
    let fields = fields(
        &app.details[app.active],
        &app.metadata_format(floating),
        app.now,
        theme,
    );
    let width = if floating {
        bounds.width.saturating_sub(4).min(90)
    } else {
        bounds.width
    };
    let rows = rows(&fields, width.saturating_sub(2), theme);
    let area = if floating {
        super::centered(
            bounds,
            width,
            u16::try_from(rows.len().saturating_add(2))
                .unwrap_or(u16::MAX)
                .min(bounds.height.saturating_sub(2)),
        )
    } else {
        bounds
    };
    let inner = Block::default().borders(Borders::ALL).inner(area);
    let limit =
        u16::try_from(rows.len().saturating_sub(usize::from(inner.height))).unwrap_or(u16::MAX);
    let scroll = if floating {
        app.overlay_scroll
    } else {
        app.metadata_scroll
    }
    .min(limit);
    let offset = label_width(&fields, inner.width) as u16;
    let controls = value_controls(&rows, inner, scroll, offset);
    Layout {
        area,
        lines: rows.into_iter().map(|(line, _, _)| line).collect(),
        controls,
        scroll,
        limit,
    }
}

fn value_controls(rows: &[Row], inner: Rect, scroll: u16, offset: u16) -> Vec<MetadataControl> {
    rows.iter()
        .skip(usize::from(scroll))
        .take(usize::from(inner.height))
        .enumerate()
        .filter_map(|(row, (_, action, length))| {
            let action = (*action)?;
            let width = (*length).min(inner.width.saturating_sub(offset));
            (width > 0).then_some(MetadataControl {
                area: Rect::new(inner.x + offset, inner.y + row as u16, width, 1),
                action,
            })
        })
        .collect()
}

pub(super) fn scroll_limit(app: &App, bounds: Rect, floating: bool) -> u16 {
    layout(
        app,
        bounds,
        &Theme::new(colored_text::ColorLevel::NoColor),
        floating,
    )
    .limit
}

pub(super) fn controls(app: &App, bounds: Rect, floating: bool) -> Vec<MetadataControl> {
    layout(
        app,
        bounds,
        &Theme::new(colored_text::ColorLevel::NoColor),
        floating,
    )
    .controls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::text::wrap_value;
    use colored_text::ColorLevel;

    #[test]
    fn wrapped_values_keep_graphemes_and_continue_under_value_column() {
        let theme = Theme::new(ColorLevel::Ansi16);
        let fields = vec![Field::plain(
            "Path",
            "界e\u{301}界e\u{301}界e\u{301}\u{1b}\n".into(),
            &theme,
        )];
        let lines: Vec<_> = rows(&fields, 13, &theme)
            .into_iter()
            .map(|(line, _, _)| line)
            .collect();
        assert!(lines.len() > 1);
        let mut restored = String::new();
        for (index, line) in lines.iter().enumerate() {
            assert!(line.width() <= 13);
            assert_eq!(line.spans[0].width(), 6);
            assert_eq!(line.spans[0].content.trim().is_empty(), index != 0);
            for span in &line.spans[1..] {
                assert!(!span.content.contains('\u{1b}'));
                restored.push_str(&span.content);
            }
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
    fn metadata_text(app: &App, floating: bool) -> String {
        use ratatui::{Terminal, backend::TestBackend};
        let width = if floating { 180 } else { 48 };
        let bounds = Rect::new(0, 0, width, 40);
        let theme = Theme::new(ColorLevel::NoColor);
        let area = layout(app, bounds, &theme, floating).area;
        let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
        terminal
            .draw(|frame| render(frame, app, bounds, &theme, floating))
            .unwrap();
        let buffer = terminal.backend().buffer();
        // Rejoin wrapped metadata values without borders or continuation padding.
        (area.y + 1..area.bottom() - 1)
            .map(|y| {
                (area.x + 1..area.right() - 1)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn totals_views_distinguish_running_complete_incomplete_and_failed_results() {
        for floating in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let mut app = App::new(root.path().into());
            app.details[0].entry = Some(crate::fs::inspect(root.path()));
            app.details[0].path = Some(root.path().into());
            app.popup = floating;
            app.sidebar = !floating;
            app.details[0].counting = true;
            assert!(metadata_text(&app, floating).contains("Calculating totals"));
            app.details[0].counting = false;
            app.details[0].totals = Some(Ok(crate::fs::Totals {
                files: 1,
                directories: 2,
                bytes: 3,
                errors: vec![],
            }));
            let complete = metadata_text(&app, floating);
            assert!(complete.contains("1 files, 2 directories, 3 B"));
            assert!(!complete.contains("INCOMPLETE") && !complete.contains("Calculating totals"));
            app.details[0]
                .totals
                .as_mut()
                .unwrap()
                .as_mut()
                .unwrap()
                .errors
                .push("unreadable descendant".into());
            let incomplete = metadata_text(&app, floating);
            assert!(incomplete.contains("1 files, 2 directories, 3 B"));
            assert!(
                incomplete.contains("INCOMPLETE") && incomplete.contains("unreadable descendant")
            );
            app.details[0].totals = Some(Err("root unavailable".into()));
            let failed = metadata_text(&app, floating);
            assert!(failed.contains("root unavailable"));
            assert!(!failed.contains("Descendants") && !failed.contains("INCOMPLETE"));
        }
    }
}
