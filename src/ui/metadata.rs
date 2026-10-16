use super::{
    clean,
    listing::{size, timestamp},
};
use crate::{
    app::App,
    color::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub(super) fn render(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    frame.render_widget(Clear, area);
    let detail = &app.details[app.active];
    let mut lines = entry_lines(detail);
    append_totals(detail, &mut lines);
    if detail.counting {
        lines.push("Calculating totals… Esc cancels".into());
    }
    lines.push("c: count/size on demand; Esc cancels/closes".into());
    let text = lines
        .iter()
        .map(|line| clean(line))
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(text)
            .scroll((if app.popup { app.overlay_scroll } else { 0 }, 0))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Metadata"))
            .style(theme.style(Role::Normal)),
        area,
    );
}

fn entry_lines(detail: &crate::app::Detail) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(entry) = &detail.entry {
        lines.extend([
            format!("Name: {}", entry.name.to_string_lossy()),
            format!("Path: {}", entry.path.display()),
            format!(
                "Canonical: {}",
                detail
                    .canonical
                    .as_ref()
                    .map_or_else(|| "unavailable".into(), |p| p.display().to_string())
            ),
            format!("Type: {:?}", entry.kind),
            format!("Size (entry): {}", size(entry.size)),
            format!("Modified UTC: {}", timestamp(entry.modified)),
            format!("Created UTC: {}", timestamp(entry.created)),
            format!("Accessed UTC: {}", timestamp(entry.accessed)),
            format!("Mode/attributes: {}", entry.attributes),
            format!(
                "Owner/group: {}",
                entry.owner.as_deref().unwrap_or("unavailable")
            ),
            format!(
                "Link target: {}",
                entry
                    .link_target
                    .as_ref()
                    .map_or_else(|| "not available".into(), |p| p.display().to_string())
            ),
        ]);
        if let Some(error) = &entry.error {
            lines.push(format!("Error: {error}"));
        }
    } else {
        lines.push(
            if detail.path.is_some() {
                "Loading metadata…"
            } else {
                "No visible entry"
            }
            .into(),
        );
    }
    lines
}

fn append_totals(detail: &crate::app::Detail, lines: &mut Vec<String>) {
    if let Some(totals) = &detail.totals {
        match totals {
            Ok(totals) => {
                lines.push(format!(
                    "Descendants: {} files, {} directories, {}{}",
                    totals.files,
                    totals.directories,
                    size(Some(totals.bytes)),
                    if totals.errors.is_empty() {
                        ""
                    } else {
                        " — INCOMPLETE"
                    }
                ));
                lines.extend(totals.errors.iter().cloned());
            }
            Err(error) => lines.push(format!("Totals: {error}")),
        }
    }
}
