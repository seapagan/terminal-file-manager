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
