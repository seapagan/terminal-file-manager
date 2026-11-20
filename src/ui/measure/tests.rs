use super::*;
use crate::{
    owner::{
        OwnerDisplay,
        windows::{Name, Ownership},
    },
    permissions::PermissionDisplay,
};

fn measure(entries: &[Entry], visible: &[usize], mode: OwnerDisplay) -> Measurements {
    Measurements::collect(
        visible.iter().map(|index| &entries[*index]),
        false,
        PermissionDisplay::default(),
        mode,
    )
}

fn owner(sid: &str, name: &str) -> Ownership {
    Ownership::Sid {
        sid: sid.into(),
        name: Name::Resolved(name.into()),
    }
}

#[test]
fn incremental_widths_match_collection_through_filter_and_display_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"x").unwrap();
    let mut entries = vec![crate::fs::inspect(&path); 3];
    for mode in [OwnerDisplay::Names, OwnerDisplay::Numeric] {
        for visible in [&[0, 1, 2][..], &[0, 2][..], &[1][..], &[][..]] {
            for entry in &mut entries {
                entry.windows_owner = owner("S-1-5-18", "Alice");
            }
            let mut incremental = measure(&entries, visible, mode);
            for next in [
                owner("S-1-5-21-123456789-1001", "DOMAIN\\Long account"),
                owner("S-1", "Li"),
                owner("S-1-5-18", "Alice"),
                Ownership::Pending,
            ] {
                for index in visible {
                    let entry = &mut entries[*index];
                    let before = entry.owner_display(mode);
                    entry.windows_owner = next.clone();
                    incremental.update_owner(&before, &entry.owner_display(mode));
                    let fresh = measure(&entries, visible, mode);
                    assert_eq!(incremental.owner, fresh.owner);
                    assert_eq!(incremental.owner_widths, fresh.owner_widths);
                    assert_eq!(
                        (incremental.name, incremental.number, incremental.attributes),
                        (fresh.name, fresh.number, fresh.attributes)
                    );
                }
            }
        }
    }
}
