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
        let text = format!("FILTER: {}", super::clean(&pane.filter));
        segments.insert(
            0,
            Line::from(Span::styled(
                super::modal::truncate(&text, width),
                theme
                    .style(Role::Warning)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )),
        );
    }
    segments.extend([
        field("Group", format!("{:?}", pane.sort.grouping), theme),
        field(
            "Hidden",
            if pane.show_hidden { "Show" } else { "Hide" }.into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_filter_precedes_decorations_and_survives_narrow_monochrome() {
        let mut app = crate::app::App::new("owned".into());
        let pane = &mut app.panes[0];
        pane.set_filter("a-long-expression".into());
        for level in [
            colored_text::ColorLevel::NoColor,
            colored_text::ColorLevel::TrueColor,
        ] {
            for width in [12, 24, 80] {
                let line = line(&pane, width, &Theme::new(level));
                assert!(line.to_string().starts_with("FILTER:"));
                assert!(line.width() <= usize::from(width));
                assert!(
                    line.spans[0]
                        .style
                        .add_modifier
                        .contains(ratatui::style::Modifier::BOLD)
                );
            }
        }
    }
}
