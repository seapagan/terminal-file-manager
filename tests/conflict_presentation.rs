use colored_text::ColorLevel;
use crossterm::event::KeyCode;
use std::time::{Duration, UNIX_EPOCH};
use terminal_file_manager::{
    color::{Role, Theme},
    fs::EntryKind,
    operations::Event,
};
mod conflict_fixture;
use conflict_fixture::{draw, edit, fixture};
#[test]
fn common_root_comparison_is_compact_and_uses_exact_subsecond_direction() {
    let (root, mut app, _) = fixture();
    let (_, g, text) = draw(&app, 100, 35, ColorLevel::NoColor);
    assert!(text.contains("File already exists"), "{text}");
    assert_eq!(text.matches("Root:").count(), 1, "{text}");
    assert!(
        text.contains(
            &std::path::Path::new("source")
                .join("conflict.txt")
                .display()
                .to_string()
        ) && text.contains(
            &std::path::Path::new("destination")
                .join("conflict.txt")
                .display()
                .to_string()
        ),
        "{text}"
    );
    assert!(text.contains("22 B") && text.contains("53 B"), "{text}");
    assert!(text.contains("Existing newer (<1 s)"), "{text}");
    edit(&mut app, |source, destination, _| {
        for entry in [source, destination] {
            let expected = expected_modified(entry.modified.unwrap());
            assert!(text.contains(&expected), "{text}");
        }
    });
    assert!(!text.contains("Identity:") && !text.contains("volume "));
    assert!(!text.contains(".050000000") && !text.contains("arrows select"));
    assert_eq!(g.operation_scroll_limit, 0);
    let _ = root;
}

fn expected_modified(modified: std::time::SystemTime) -> String {
    let nanos = modified.duration_since(UNIX_EPOCH).unwrap().as_nanos() as i128;
    let date = time::OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        date.year(),
        date.month() as u8,
        date.day(),
        date.hour(),
        date.minute(),
        date.second()
    )
}

#[test]
fn newer_direction_uses_full_timestamps_and_respects_availability() {
    let (_root, mut app, _) = fixture();
    let base = UNIX_EPOCH + Duration::from_secs(1793038260);
    for (incoming, existing, available, expected) in [
        (
            Some(base + Duration::from_millis(90)),
            Some(base + Duration::from_millis(50)),
            true,
            "Incoming newer (<1 s)",
        ),
        (Some(base), Some(base), true, "Same modified time"),
        (
            Some(base),
            Some(base + Duration::from_secs(2)),
            true,
            "Existing newer",
        ),
        (None, Some(base), false, "Cannot compare reliably"),
        (Some(base), None, false, "Cannot compare reliably"),
        (Some(base), Some(base), false, "Cannot compare reliably"),
        (
            Some(base + Duration::from_nanos(1)),
            Some(base),
            false,
            "Cannot compare reliably",
        ),
        (
            Some(base),
            Some(base + Duration::from_nanos(1)),
            false,
            "Cannot compare reliably",
        ),
    ] {
        edit(&mut app, |s, d, a| {
            s.modified = incoming;
            d.modified = existing;
            *a = available;
        });
        let (_, _, text) = draw(&app, 120, 40, ColorLevel::NoColor);
        assert!(text.contains(expected), "{text}");
        assert!(!text.contains(".050000000"));
        assert!(!text.contains("Scope:"), "{text}");
        assert!(!text.contains("N: replace only"), "{text}");
        assert_native_times(&app, incoming, existing);
    }
}

fn assert_native_times(
    app: &terminal_file_manager::app::App,
    incoming: Option<std::time::SystemTime>,
    existing: Option<std::time::SystemTime>,
) {
    if let Event::Conflict {
        source,
        destination,
        ..
    } = &app.operations.conflicts.front().unwrap().event
    {
        assert_eq!(source.modified, incoming);
        assert_eq!(destination.modified, existing);
    }
}

#[test]
fn comparison_switches_to_stacked_rows_and_keeps_semantic_labels() {
    let (_root, app, _) = fixture();
    for width in [40, 64, 65, 80, 120] {
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            let (terminal, g, text) = draw(&app, width, 40, level);
            assert!(
                text.contains("Incoming (source)") && text.contains("Existing (destination)"),
                "{text}"
            );
            let buffer = terminal.backend().buffer();
            assert_key_value_styles(buffer, level, &g);
            let header_y = |heading: &str| {
                (0..40)
                    .find(|&y| {
                        (0..width)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>()
                            .contains(heading)
                    })
                    .unwrap()
            };
            let incoming = header_y("Incoming (source)");
            let existing = header_y("Existing (destination)");
            assert!(incoming <= existing);
            if width == 40 {
                assert!(incoming < existing, "narrow comparison must stack");
            } else if width == 120 {
                assert_eq!(incoming, existing, "wide comparison must use columns");
            }
            if width == 40 {
                assert!(g.operation_controls.iter().all(|b| b.area.right() <= width));
            }
        }
    }
}

fn assert_key_value_styles(
    buffer: &ratatui::buffer::Buffer,
    level: ColorLevel,
    g: &terminal_file_manager::ui::Geometry,
) {
    let width = buffer.area.width;
    let label = (0..40)
        .find_map(|y| {
            (0..width.saturating_sub(5))
                .find(|&x| {
                    (x..x + 5)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        == "Path:"
                })
                .map(|x| (x, y))
        })
        .unwrap();
    let style = Theme::new(level).style(Role::Label);
    assert_eq!(
        buffer[label].fg,
        style.fg.unwrap_or(ratatui::style::Color::Reset)
    );
    assert_eq!(buffer[label].modifier, style.add_modifier);
    assert_eq!(g.operation_target(label.0, label.1), None);
    let value = (label.0 + 10, label.1);
    assert_eq!(g.operation_target(value.0, value.1), None);
    assert_eq!(
        buffer[value].fg,
        Theme::new(level)
            .style(Role::Value)
            .fg
            .unwrap_or(ratatui::style::Color::Reset)
    );
    assert_eq!(
        buffer[value].modifier,
        Theme::new(level).style(Role::Value).add_modifier
    );
}

#[test]
fn long_paths_keep_both_ends_without_changing_native_operands() {
    let (_root, mut app, _) = fixture();
    let base = std::path::PathBuf::from("/home/資料/a-long-session-name/visual");
    edit(&mut app, |s, d, _| {
        s.path = base.join(format!("source/{}/conflict.txt", "intermediate/".repeat(8)));
        d.path = base.join(format!(
            "destination/{}/conflict.txt",
            "intermediate/".repeat(8)
        ));
    });
    let frozen = match &app.operations.conflicts.front().unwrap().event {
        Event::Conflict {
            source,
            destination,
            ..
        } => (source.path.clone(), destination.path.clone()),
        _ => unreachable!(),
    };
    for width in [40, 80, 120] {
        let (_, _, text) = draw(&app, width, 50, ColorLevel::NoColor);
        assert!(text.contains("Root:"));
        assert!(text.contains("visual"));
        assert!(
            text.contains("source/") && text.contains("destination"),
            "{width}: {text}"
        );
        assert!(text.contains("conflict.txt"));
    }
    if let Event::Conflict {
        source,
        destination,
        ..
    } = &app.operations.conflicts.front().unwrap().event
    {
        assert_eq!((&source.path, &destination.path), (&frozen.0, &frozen.1));
    }
}

#[test]
fn unrelated_roots_and_different_names_are_not_falsely_grouped() {
    let (_root, mut app, _) = fixture();
    edit(&mut app, |s, d, _| {
        s.path = "/alpha/incoming.txt".into();
        s.name = "incoming.txt".into();
        d.path = "/beta/existing.txt".into();
        d.name = "existing.txt".into();
    });
    let (_, _, text) = draw(&app, 120, 40, ColorLevel::NoColor);
    assert!(!text.contains("Root:"));
    assert!(text.contains("incoming.txt") && text.contains("existing.txt"));
    assert!(text.contains("/alpha/") && text.contains("/beta/"));
}

#[test]
fn single_component_elision_keeps_a_recognisable_beginning_and_end() {
    let (_root, mut app, _) = fixture();
    edit(&mut app, |s, d, _| {
        s.path = format!("/start-source/{}-tail.txt", "界e\u{301}".repeat(60)).into();
        d.path = format!("/start-existing/{}-tail.txt", "界e\u{301}".repeat(60)).into();
        s.name = s.path.file_name().unwrap().to_owned();
        d.name = d.path.file_name().unwrap().to_owned();
    });
    let (_, _, text) = draw(&app, 80, 50, ColorLevel::NoColor);
    assert!(text.contains("/start-"), "{text}");
    assert!(text.contains("-tail.txt"), "{text}");
    assert!(!text.contains('�'));
}

#[test]
fn conflict_type_values_are_descriptive_without_listing_markers() {
    let (_root, mut app, _) = fixture();
    for (kind, name) in [
        (EntryKind::File, "File"),
        (EntryKind::Directory, "Directory"),
        (EntryKind::Link, "Link"),
        (EntryKind::Junction, "Junction"),
        (EntryKind::ReparsePoint, "Reparse point"),
        (EntryKind::Socket, "Socket"),
        (EntryKind::Fifo, "FIFO"),
        (EntryKind::Device, "Device"),
        (EntryKind::Unknown, "Unknown"),
    ] {
        edit(&mut app, |s, d, _| {
            s.kind = kind;
            d.kind = kind;
        });
        assert_type_values(&app, &format!("{name} {name}"));
    }
}

#[test]
fn mismatched_type_names_preserve_disabled_overwrite_choices() {
    let (_root, mut app, _) = fixture();
    edit(&mut app, |_, d, available| {
        d.kind = EntryKind::Directory;
        *available = false;
    });
    assert_type_values(&app, "File Directory");
    for width in [40, 80, 120] {
        let (_, geometry, _) = draw(&app, width, 40, ColorLevel::NoColor);
        assert!(!geometry.operation_enabled(KeyCode::Char('r')));
        assert!(!geometry.operation_enabled(KeyCode::Char('n')));
    }
}

fn assert_type_values(app: &terminal_file_manager::app::App, expected: &str) {
    for width in [40, 80, 120] {
        let (terminal, _, text) = draw(app, width, 40, ColorLevel::NoColor);
        let values = terminal
            .backend()
            .buffer()
            .content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .filter_map(|row| {
                row.split_once("Type:").map(|(_, values)| {
                    values
                        .trim_matches([' ', '│'])
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(values, expected, "{text}");
    }
}

#[test]
fn non_file_types_never_gain_regular_file_sizes() {
    let (_root, mut app, _) = fixture();
    for kind in [
        EntryKind::Link,
        EntryKind::Junction,
        EntryKind::Device,
        EntryKind::Socket,
        EntryKind::Unknown,
    ] {
        edit(&mut app, |s, d, a| {
            s.kind = kind;
            d.kind = kind;
            *a = false;
        });
        let (_, g, text) = draw(&app, 120, 40, ColorLevel::NoColor);
        assert!(!text.contains("22 B") && !text.contains("53 B"));
        assert!(text.contains("—"));
        assert!(!g.operation_enabled(KeyCode::Char('n')));
    }
}

#[cfg(unix)]
#[test]
fn native_non_utf8_names_controls_and_symlink_spelling_stay_in_the_model() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let (_root, mut app, _) = fixture();
    let name = OsString::from_vec(b"bad-\xff\x1b\n".to_vec());
    edit(&mut app, |s, d, _| {
        s.name = name.clone();
        d.name = name.clone();
        s.path = std::path::PathBuf::from("/lexical/link/../source").join(&name);
        d.path = std::path::PathBuf::from("/lexical/destination").join(&name);
    });
    let (_, _, text) = draw(&app, 120, 40, ColorLevel::NoColor);
    assert!(text.contains("bad-\\xFF"), "{text}");
    assert!(!text.contains('\u{1b}'));
    if let Event::Conflict {
        source,
        destination,
        ..
    } = &app.operations.conflicts.front().unwrap().event
    {
        assert_eq!(source.name, name);
        assert_eq!(destination.name, name);
        assert!(
            source
                .path
                .as_os_str()
                .as_encoded_bytes()
                .windows(2)
                .any(|w| w == b"..")
        );
    }
}

#[test]
fn tiny_conflicts_keep_all_control_regions_inside_the_terminal() {
    let (_root, app, _) = fixture();
    for width in [0, 1, 12, 20, 40, 80, 120] {
        for height in [0, 1, 5, 12, 15, 24] {
            let (_, g, _) = draw(&app, width, height, ColorLevel::NoColor);
            assert!(
                g.operation_controls
                    .iter()
                    .all(|b| b.area.right() <= width && b.area.bottom() <= height)
            );
        }
    }
}

#[cfg(windows)]
#[test]
fn native_drive_and_unc_roots_do_not_invent_a_shared_namespace() {
    let (_root, mut app, _) = fixture();
    for (incoming, existing, common) in [
        (
            r"C:\visual\source\conflict.txt",
            r"D:\visual\destination\conflict.txt",
            false,
        ),
        (
            r"\\server\one\source\conflict.txt",
            r"\\server\two\destination\conflict.txt",
            false,
        ),
        (
            r"C:\visual\source\conflict.txt",
            r"C:\visual\destination\conflict.txt",
            true,
        ),
    ] {
        edit(&mut app, |s, d, _| {
            s.path = incoming.into();
            d.path = existing.into();
        });
        let (_, _, text) = draw(&app, 120, 40, ColorLevel::NoColor);
        assert_eq!(text.contains("Root:"), common, "{text}");
        if let Event::Conflict {
            source,
            destination,
            ..
        } = &app.operations.conflicts.front().unwrap().event
        {
            assert_eq!(source.path.as_os_str(), incoming);
            assert_eq!(destination.path.as_os_str(), existing);
        }
    }
}

#[test]
fn conflict_width_tracks_content_before_eliding_paths() {
    let (_root, mut app, _) = fixture();
    for width in [120, 160] {
        let (short, _, _) = draw(&app, width, 50, ColorLevel::NoColor);
        edit(&mut app, |s, d, _| {
            s.path = "/visual/source/nested/incoming-folder/conflict.txt".into();
            d.path = "/visual/destination/nested/existing-folder/conflict.txt".into();
        });
        let (long, _, text) = draw(&app, width, 50, ColorLevel::NoColor);
        assert!(
            rendered_conflict_width(&long) > rendered_conflict_width(&short),
            "{text}"
        );
        assert!(
            text.contains("source/nested/incoming-folder/conflict.txt"),
            "{text}"
        );
        assert!(
            text.contains("destination/nested/existing-folder/conflict.txt"),
            "{text}"
        );
        assert!(!text.contains('…'), "{text}");
        edit(&mut app, |s, d, _| {
            s.path = "/visual/source/conflict.txt".into();
            d.path = "/visual/destination/conflict.txt".into();
        });
    }
}

fn rendered_conflict_width(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> usize {
    let b = terminal.backend().buffer();
    let row = (0..b.area.height)
        .map(|y| {
            (0..b.area.width)
                .map(|x| b[(x, y)].symbol())
                .collect::<String>()
        })
        .find(|row| row.contains("┌─File already exists"))
        .unwrap();
    let start = row.find('┌').unwrap();
    let end = row.rfind('┐').unwrap();
    ratatui::text::Line::raw(&row[start..end]).width() + 1
}
