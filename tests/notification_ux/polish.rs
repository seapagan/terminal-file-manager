use super::*;

#[test]
fn notification_cards_have_no_box_edges_and_keep_continuous_accents_and_padding() {
    let mut c = controller();
    c.app.notify(Severity::Error, "Readable message");
    let (buffer, geometry) = draw(&c, 80);
    let area = geometry.toasts[0].area;
    for y in area.y..area.bottom() {
        assert_eq!(buffer[(area.x, y)].symbol(), "▌");
        assert_eq!(buffer[(area.x + 1, y)].symbol(), " ");
        assert_eq!(buffer[(area.right() - 1, y)].symbol(), " ");
    }
    for y in [area.y, area.bottom() - 1] {
        for x in area.x + 1..area.right() {
            assert_eq!(buffer[(x, y)].symbol(), " ");
        }
    }
    assert_eq!(buffer[(area.x + 2, area.y + 1)].symbol(), "E");
    assert!(text(&buffer, area).contains("Readable message"));
}

#[test]
fn pane_prefixes_leave_summaries_but_remain_in_event_details() {
    let mut c = controller();
    let detail = "Pane 2: Directory could not be read";
    c.app.notify(Severity::Warning, detail);
    let (buffer, geometry) = draw(&c, 80);
    let card = text(&buffer, geometry.toasts[0].area);
    assert!(!card.contains("Pane 2:"));
    assert!(card.contains("Directory could not be read"));
    assert_eq!(c.app.notifications.history[0].text, detail);
    c.app.notifications.open = true;
    c.app.notifications.details = true;
    let (buffer, geometry) = draw(&c, 80);
    assert!(text(&buffer, geometry.notification_history.unwrap().shell.body).contains(detail));
}

#[test]
fn history_summary_uses_every_cell_left_after_selection_severity_and_time() {
    let mut c = controller();
    c.app.notify(Severity::Error, "placeholder");
    c.app.notifications.open = true;
    let (_, geometry) = draw(&c, 80);
    let body = geometry.notification_history.unwrap().shell.body;
    // Selection/severity, a two-cell gap and an eight-cell timestamp reserve eighteen cells.
    let available = usize::from(body.width) - 18;
    let summary = "x".repeat(available);
    c.app.notifications.history[0]
        .presentation
        .message
        .clone_from(&summary);
    let (buffer, geometry) = draw(&c, 80);
    let row = text(&buffer, geometry.notification_history.unwrap().shell.body);
    assert!(row.contains(&summary), "{row}");
    assert!(row.ends_with(" now"), "{row}");
    assert!(!row.contains('…'), "{row}");
}

#[test]
fn stacked_cards_keep_a_blank_row_between_surfaces() {
    let mut c = controller();
    for message in ["First", "Second", "Third"] {
        c.app.notify(Severity::Warning, message);
    }
    let (_, geometry) = draw(&c, 120);
    assert_eq!(geometry.toasts.len(), 3);
    for pair in geometry.toasts.windows(2) {
        assert!(pair[1].area.bottom() < pair[0].area.y);
    }
}

#[test]
fn history_wide_summaries_keep_the_timestamp_at_the_right_edge_on_resize() {
    let mut c = controller();
    c.app.notify(Severity::Error, "placeholder");
    c.app.notifications.open = true;
    for (width, time_width) in [(20, 3), (40, 8), (80, 8), (120, 8)] {
        let (_, geometry) = draw(&c, width);
        let body = geometry.notification_history.unwrap().shell.body;
        assert_eq!(body.height, 1, "width {width}");
        let available = usize::from(body.width - time_width) - 10;
        let summary = "界".repeat(available / 2) + if available % 2 == 1 { "z" } else { "" };
        c.app.notifications.history[0].presentation.message = summary;
        let (buffer, _) = draw(&c, width);
        for offset in 0..available / 2 {
            assert_eq!(
                buffer[(body.x + 8 + offset as u16 * 2, body.y)].symbol(),
                "界"
            );
        }
        let time = ratatui::layout::Rect::new(body.right() - time_width, body.y, time_width, 1);
        assert_eq!(text(&buffer, time).trim(), "now");
        assert!(!text(&buffer, body).contains('…'));
    }
}

#[test]
fn compact_history_ages_keep_relative_units_and_future_clock_evidence() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(172800);
    for (offset, expected) in [
        (0i64, "now"),
        (-1, "1s"),
        (-60, "1m"),
        (-3600, "1h"),
        (-86400, "1d"),
        (60, "in 1m"),
    ] {
        let mut c = controller();
        c.app.now = now;
        let duration = Duration::from_secs(offset.unsigned_abs());
        let created = if offset < 0 {
            now - duration
        } else {
            now + duration
        };
        c.app
            .notifications
            .notify("Message".into(), created, Severity::Info);
        c.app.notifications.open = true;
        let (buffer, geometry) = draw(&c, 80);
        let row = text(&buffer, geometry.notification_history.unwrap().shell.body);
        assert!(row.ends_with(expected), "{row}");
    }
}
