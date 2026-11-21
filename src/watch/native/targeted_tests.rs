use super::*;
use notify::event::{AccessKind, AccessMode, CreateKind, ModifyKind};
#[test]
fn only_precise_immediate_child_metadata_hints_can_avoid_full_reconciliation() {
    let raw = Path::new("/alias/current");
    let physical = Path::new("/real/current");
    for kind in [
        EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
        EventKind::Access(AccessKind::Close(AccessMode::Write)),
    ] {
        let event = Event::new(kind).add_path(physical.join("known"));
        assert_eq!(
            precise_children(&event, raw, physical),
            Some(vec![raw.join("known")])
        );
    }
    for (kind, path) in [
        (EventKind::Create(CreateKind::File), raw.join("new")),
        (EventKind::Modify(ModifyKind::Any), raw.join("known")),
        (
            EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::Any)),
            raw.join(".gitignore"),
        ),
        (
            EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::Any)),
            raw.to_path_buf(),
        ),
    ] {
        assert!(precise_children(&Event::new(kind).add_path(path), raw, physical).is_none());
    }
}
