//! Shared label/value presentation for the metadata sidebar and floating popup.
#[cfg(test)]
mod cached_tests;
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
            self.action = entry.mode.map(|_| Action::Permissions);
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
    inner: Rect,
    shell: Option<super::modal::Layout<Action>>,
    lines: Vec<Line<'static>>,
    controls: Vec<MetadataControl>,
    scroll: u16,
    limit: u16,
}

pub(super) fn render(frame: &mut Frame, app: &App, bounds: Rect, theme: &Theme, floating: bool) {
    let layout = layout(app, bounds, theme, floating);
    if let Some(shell) = &layout.shell {
        super::modal::render_shell(frame, layout.area, "Metadata — ↑/↓ or wheel", theme);
        frame.render_widget(
            Paragraph::new(layout.lines).scroll((layout.scroll, 0)),
            layout.inner,
        );
        super::modal::actions::render(frame, shell, theme, None);
    } else {
        frame.render_widget(Clear, layout.area);
        frame.render_widget(
            Paragraph::new(layout.lines)
                .scroll((layout.scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Metadata — wheel scrolls")
                        .border_style(theme.style(Role::Header)),
                ),
            layout.area,
        );
    }
}

fn fields(
    detail: &Detail,
    pane: &MetadataFormat,
    now: std::time::SystemTime,
    theme: &Theme,
    floating: bool,
) -> Vec<Field> {
    if let Some(error) = &detail.stale {
        return stale_fields(error, theme, floating);
    }
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
    fields.push((
        "Keys",
        if floating {
            "c: count/size on demand; Esc cancels/closes"
        } else {
            "c: count/size on demand; Esc cancels the count"
        }
        .into(),
    ));
    fields
        .into_iter()
        .map(|(label, value)| Field::plain(label, value, theme).interactive(detail, pane, theme))
        .collect()
}

fn stale_fields(error: &str, theme: &Theme, floating: bool) -> Vec<Field> {
    vec![
        Field::plain("Status", "Stale directory listing".into(), theme),
        Field::plain("Error", error.into(), theme),
        Field::plain(
            "Keys",
            if floating {
                "Esc closes"
            } else {
                "r: refresh; reopen to select a new context"
            }
            .into(),
            theme,
        ),
    ]
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
        return vec![("Status", "No visible entry".into())];
    };
    let mut fields = vec![
        ("Name", entry.name.to_string_lossy().into_owned()),
        ("Path", entry.path.display().to_string()),
        (
            "Canonical",
            detail.canonical.as_ref().map_or_else(
                || {
                    if detail.canonical_pending {
                        "Resolving…"
                    } else {
                        "Unavailable"
                    }
                    .into()
                },
                |p| p.display().to_string(),
            ),
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
        floating,
    );
    let (shell, measured_rows) = if floating {
        let (shell, rows) = popup_layout(&fields, bounds, theme, &app.details[app.active]);
        (Some(shell), rows)
    } else {
        (None, Vec::new())
    };
    let (area, inner, rows) = if let Some(shell) = &shell {
        (shell.outer, shell.body, measured_rows)
    } else {
        (
            bounds,
            Block::default().borders(Borders::ALL).inner(bounds),
            rows(&fields, bounds.width.saturating_sub(2), theme),
        )
    };
    let limit =
        u16::try_from(rows.len().saturating_sub(usize::from(inner.height))).unwrap_or(u16::MAX);
    let scroll = if floating {
        app.overlay_scroll
    } else {
        app.metadata_scroll
    }
    .min(limit);
    let offset = label_width(&fields, inner.width) as u16;
    let mut controls = value_controls(&rows, inner, scroll, offset);
    if let Some(shell) = &shell {
        controls.extend(shell.actions.iter().map(|region| MetadataControl {
            area: region.area,
            action: region.action.intent,
        }));
    }
    Layout {
        area,
        inner,
        shell,
        lines: rows.into_iter().map(|(line, _, _)| line).collect(),
        controls,
        scroll,
        limit,
    }
}

fn popup_layout(
    fields: &[Field],
    bounds: Rect,
    theme: &Theme,
    detail: &Detail,
) -> (super::modal::Layout<Action>, Vec<Row>) {
    let mut measured_rows = Vec::new();
    let shell = {
        let actions = [super::modal::actions::Action::custom(
            "Esc Close",
            Action::Cancel,
            crossterm::event::KeyCode::Esc,
        )];
        super::modal::Spec {
            title: "Metadata — ↑/↓ or wheel",
            content_width: label_width(fields, u16::MAX)
                + fields
                    .iter()
                    .map(|field| field.value.width())
                    .max()
                    .unwrap_or(0),
            pinned_rows: 0,
            footer_rows: 0,
            actions: &actions,
        }
        .measure(bounds, |width| {
            measured_rows = rows(fields, width, theme);
            if let Some(entry) = detail.entry.as_ref().filter(|_| detail.stale.is_none()) {
                let row = super::modal::entry_row::<()>(entry, std::path::Path::new(&entry.name));
                let row = row.wrap(width).remove(0).line(width, theme);
                measured_rows.insert(0, (row, None, 0));
            }
            measured_rows.len()
        })
    };
    (shell, measured_rows)
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
mod tests;
