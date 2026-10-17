//! Counts describe the visible directory and the selected working scope separately.
use crate::{
    app::SelectionCounts,
    color::{Role, Theme},
};
use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
};

fn value(count: usize, theme: &Theme, role: Role) -> Span<'static> {
    Span::styled(
        count.to_string(),
        theme.style(role).add_modifier(Modifier::BOLD),
    )
}

fn selection(counts: SelectionCounts, compact: bool, theme: &Theme) -> Line<'static> {
    if counts.visible + counts.excluded() == 0 {
        return Line::from(Span::styled("No selection", theme.style(Role::Normal)));
    }
    let mut spans = Vec::new();
    if !compact {
        spans.push(Span::styled("Selected: ", theme.style(Role::Label)));
    }
    spans.push(value(counts.visible, theme, Role::Value));
    spans.push(Span::raw(if compact { " sel" } else { " visible" }));
    if counts.excluded() > 0 {
        spans.push(Span::raw(" +"));
        if !compact {
            spans.push(Span::raw(" "));
        }
        spans.push(value(counts.excluded(), theme, Role::Warning));
        spans.push(Span::raw(if compact { " excl" } else { " excluded" }));
    }
    Line::from(spans)
}

fn breakdown(counts: SelectionCounts, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (count, label) in [
        (counts.hidden, " hidden"),
        (counts.ignored, " ignored"),
        (counts.filtered, " filtered"),
    ] {
        if count == 0 {
            continue;
        }
        spans.push(Span::raw(if spans.is_empty() { " (" } else { " · " }));
        spans.push(value(count, theme, Role::Warning));
        spans.push(Span::raw(label));
    }
    if !spans.is_empty() {
        spans.push(Span::raw(")"));
    }
    spans
}

fn together(items: &Line<'static>, selection: &Line<'static>) -> Line<'static> {
    let mut spans = items.spans.clone();
    spans.push(Span::raw("  │  "));
    spans.extend(selection.spans.clone());
    Line::from(spans)
}

pub(super) fn lines(
    items: usize,
    counts: SelectionCounts,
    width: u16,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let items = Line::from(vec![value(items, theme, Role::Value), Span::raw(" items")]);
    let selected = selection(counts, false, theme);
    let mut detailed = selected.clone();
    detailed.spans.extend(breakdown(counts, theme));
    for selected in [detailed, selected, selection(counts, true, theme)] {
        let combined = together(&items, &selected);
        if combined.width() <= usize::from(width) {
            return vec![combined];
        }
    }
    vec![items, selection(counts, true, theme)]
}

pub(crate) fn row_count(items: usize, counts: SelectionCounts, area: Rect) -> usize {
    let summary = lines(
        items,
        counts,
        area.width.saturating_sub(2),
        &Theme::new(colored_text::ColorLevel::NoColor),
    );
    usize::from(area.height.saturating_sub(4 + summary.len() as u16))
}
