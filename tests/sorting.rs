//! Deterministic cached metadata policies, with independent grouping and reversal.
use std::{
    fs,
    time::{Duration, UNIX_EPOCH},
};
use terminal_file_manager::{
    fs::{Entry, EntryKind, inspect},
    listing::{Grouping, Sort, SortPolicy},
};

fn entries() -> (tempfile::TempDir, Vec<Entry>) {
    let root = tempfile::tempdir().unwrap();
    let entries = [
        ("file2.rs", 2, 4),
        ("file10.txt", 10, 1),
        ("file1.rs", 1, 10),
    ]
    .into_iter()
    .map(|(name, size, created)| {
        let path = root.path().join(name);
        fs::write(&path, "fixture").unwrap();
        let mut entry = inspect(&path);
        entry.size = Some(size);
        entry.modified = Some(UNIX_EPOCH + Duration::from_secs(size));
        entry.created = Some(UNIX_EPOCH + Duration::from_secs(created));
        entry
    })
    .collect();
    (root, entries)
}

#[test]
fn every_sort_mode_obeys_its_metadata_direction_and_stable_ties() {
    let (_root, original) = entries();
    for (sort, expected) in [
        (Sort::Natural, ["file1.rs", "file2.rs", "file10.txt"]),
        (Sort::Name, ["file1.rs", "file10.txt", "file2.rs"]),
        (Sort::Extension, ["file1.rs", "file2.rs", "file10.txt"]),
        (Sort::Size, ["file10.txt", "file2.rs", "file1.rs"]),
        (Sort::Modified, ["file10.txt", "file2.rs", "file1.rs"]),
        (Sort::Created, ["file1.rs", "file2.rs", "file10.txt"]),
        (Sort::Version, ["file1.rs", "file2.rs", "file10.txt"]),
        (Sort::Unsorted, ["file2.rs", "file10.txt", "file1.rs"]),
    ] {
        let mut entries = original.clone();
        SortPolicy {
            sort,
            grouping: Grouping::Mixed,
            reverse: false,
        }
        .apply(&mut entries);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.name.to_str().unwrap())
                .collect::<Vec<_>>(),
            expected,
            "{sort:?}"
        );
    }
}

#[test]
fn equal_metadata_sorts_by_native_name_independently_of_enumeration_order() {
    let (_root, mut original) = entries();
    for entry in &mut original {
        entry.size = Some(7);
        entry.modified = Some(UNIX_EPOCH);
        entry.created = Some(UNIX_EPOCH);
    }
    for sort in [Sort::Size, Sort::Modified, Sort::Created] {
        for reverse in [false, true] {
            let mut entries = original.clone();
            SortPolicy {
                sort,
                grouping: Grouping::Mixed,
                reverse,
            }
            .apply(&mut entries);
            let names: Vec<_> = entries
                .iter()
                .map(|entry| entry.name.to_str().unwrap())
                .collect();
            let expected = if reverse {
                ["file2.rs", "file10.txt", "file1.rs"]
            } else {
                ["file1.rs", "file10.txt", "file2.rs"]
            };
            assert_eq!(names, expected, "{sort:?} reverse={reverse}");
        }
    }
}

#[test]
fn type_sort_and_grouping_do_not_confuse_reversal_with_directory_position() {
    let (_root, mut entries) = entries();
    entries[0].kind = EntryKind::Link;
    entries[1].kind = EntryKind::Directory;
    entries[1].enterable = true;
    SortPolicy {
        sort: Sort::Kind,
        grouping: Grouping::Mixed,
        reverse: false,
    }
    .apply(&mut entries);
    assert_eq!(
        entries.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [EntryKind::Directory, EntryKind::File, EntryKind::Link]
    );
    for reverse in [false, true] {
        SortPolicy {
            sort: Sort::Size,
            grouping: Grouping::Last,
            reverse,
        }
        .apply(&mut entries);
        assert!(entries.last().unwrap().enterable);
        SortPolicy {
            sort: Sort::Unsorted,
            grouping: Grouping::Mixed,
            reverse,
        }
        .apply(&mut entries);
        assert!(entries.last().unwrap().enterable);
    }
}

#[test]
fn unsorted_transition_restores_snapshot_order_without_losing_cursor_or_marks() {
    use terminal_file_manager::{app::App, fs::Snapshot};
    let (root, mut original) = entries();
    original[1].kind = EntryKind::Directory;
    original[1].enterable = true;
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(Snapshot {
            path: root.path().to_path_buf(),
            entries: original.clone(),
            errors: vec![],
        }),
    );
    let pane = &mut app.panes[0];
    pane.select_all();
    pane.move_cursor(1, 10);
    let cursor = pane.cursor.clone();
    for grouping in [Grouping::Mixed, Grouping::First, Grouping::Last] {
        pane.sort = SortPolicy {
            sort: Sort::Size,
            grouping,
            reverse: true,
        };
        pane.resort();
        pane.sort.sort = Sort::Unsorted;
        pane.resort();
        let expected = match grouping {
            Grouping::Mixed => ["file2.rs", "file10.txt", "file1.rs"],
            Grouping::First => ["file10.txt", "file2.rs", "file1.rs"],
            Grouping::Last => ["file2.rs", "file1.rs", "file10.txt"],
        };
        assert_eq!(
            pane.entries
                .iter()
                .map(|e| e.name.to_str().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(pane.cursor, cursor);
        assert_eq!(pane.marks.len(), original.len());
    }
}

#[test]
fn refreshed_snapshot_replaces_the_remembered_filesystem_order() {
    use terminal_file_manager::{app::App, fs::Snapshot};
    let (root, mut original) = entries();
    let mut app = App::new(root.path().to_path_buf());
    app.panes[0].sort.sort = Sort::Unsorted;
    app.panes[0].sort.grouping = Grouping::Mixed;
    for _ in 0..2 {
        let request = app.request_listing(0, root.path().to_path_buf());
        app.complete_listing(
            0,
            request.generation,
            Ok(Snapshot {
                path: root.path().to_path_buf(),
                entries: original.clone(),
                errors: vec![],
            }),
        );
        assert_eq!(
            app.panes[0]
                .entries
                .iter()
                .map(|e| &e.path)
                .collect::<Vec<_>>(),
            original.iter().map(|e| &e.path).collect::<Vec<_>>()
        );
        original.reverse();
    }
}
