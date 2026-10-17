//! Whole policy segments yield to width without truncating labels or values.
use crate::{
    app::Pane,
    color::{Role, Theme},
};
use ratatui::text::{Line, Span};

fn field(label: &str, value: String, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), theme.style(Role::Label)),
        Span::styled(value, theme.style(Role::Value)),
    ])
}

pub(super) fn line(pane: &Pane, width: u16, theme: &Theme) -> Line<'static> {
    let mut segments = vec![
        Line::from(Span::styled(
            super::clean(&pane.profile_label()),
            theme.style(Role::Value),
        )),
        field("Sort", format!("{:?}", pane.sort.sort), theme),
    ];
    if pane.sort.reverse {
        segments.push(field("Reverse", "On".into(), theme));
    }
    if !pane.filter.is_empty() {
        segments.push(field("Filter", super::clean(&pane.filter), theme));
    }
    segments.extend([
        field("Group", format!("{:?}", pane.sort.grouping), theme),
        field(
            "Hidden",
            if pane.show_hidden { "On" } else { "Off" }.into(),
            theme,
        ),
        field("Ignore", format!("{:?}", pane.ignore), theme),
    ]);
    let mut line = Line::default();
    for segment in segments {
        let gap = if line.spans.is_empty() { 0 } else { 3 };
        if line.width() + gap + segment.width() > usize::from(width) {
            continue;
        }
        if gap > 0 {
            line.spans.push(Span::raw(" │ "));
        }
        line.spans.extend(segment.spans);
    }
    line
}
