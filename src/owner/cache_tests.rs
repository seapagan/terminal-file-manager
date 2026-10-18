use super::*;
use crate::fs::{self, IgnoreContext};
use std::{
    cell::RefCell, collections::HashMap, os::unix::fs::MetadataExt, sync::atomic::AtomicBool,
};

type Answer = nix::Result<Option<String>>;
struct Lookups {
    answers: [HashMap<u32, Answer>; 2],
    calls: [Vec<u32>; 2],
    labels: [String; 2],
}
thread_local! { static LOOKUPS: RefCell<Option<Lookups>> = const { RefCell::new(None) }; }

pub(super) fn lookup(group: bool, id: u32) -> Option<Answer> {
    LOOKUPS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut()?;
        let index = usize::from(group);
        state.calls[index].push(id);
        Some(
            state.answers[index]
                .get(&id)
                .cloned()
                .unwrap_or_else(|| Ok(Some(state.labels[index].clone()))),
        )
    })
}

fn controlled(test: impl FnOnce()) {
    LOOKUPS.with(|slot| {
        assert!(slot.borrow().is_none());
        slot.replace(Some(Lookups {
            answers: Default::default(),
            calls: Default::default(),
            labels: ["user".into(), "group".into()],
        }));
    });
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            LOOKUPS.with(|slot| slot.take());
        }
    }
    let _reset = Reset;
    test();
}

fn calls() -> [Vec<u32>; 2] {
    LOOKUPS.with(|slot| slot.borrow().as_ref().unwrap().calls.clone())
}

#[test]
fn listing_resolves_repeated_ids_once_and_starts_fresh_on_the_next_operation() {
    let root = tempfile::tempdir().unwrap();
    for name in ["one", "two", "three"] {
        std::fs::write(root.path().join(name), b"x").unwrap();
    }
    let metadata = std::fs::metadata(root.path()).unwrap();
    let (uid, gid) = (metadata.uid(), metadata.gid());
    controlled(|| {
        let first = fs::list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(first.entries.len(), 3);
        for entry in &first.entries {
            let owner = entry.owner.as_ref().unwrap();
            assert_eq!((owner.uid, owner.gid), (uid, gid));
            assert_eq!(owner.display(OwnerDisplay::Names), "user:group");
            assert_eq!(owner.display(OwnerDisplay::Numeric), format!("{uid}:{gid}"));
        }
        assert_eq!(calls(), [vec![uid], vec![gid]]);
        LOOKUPS.with(|slot| {
            slot.borrow_mut().as_mut().unwrap().labels = ["fresh-user".into(), "fresh-group".into()]
        });
        let second = fs::list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            second
                .entries
                .iter()
                .all(|entry| entry.owner_display(OwnerDisplay::Names) == "fresh-user:fresh-group")
        );
        assert_eq!(calls(), [vec![uid, uid], vec![gid, gid]]);
        assert_eq!(
            first.entries[0].owner_display(OwnerDisplay::Names),
            "user:group"
        );
    });
}

#[test]
fn recursive_totals_share_resolution_across_directories_but_not_operations() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("a/b")).unwrap();
    for name in ["one", "a/two", "a/b/three"] {
        std::fs::write(root.path().join(name), b"x").unwrap();
    }
    let metadata = std::fs::metadata(root.path()).unwrap();
    let (uid, gid) = (metadata.uid(), metadata.gid());
    controlled(|| {
        for expected in 1..=2 {
            let result = fs::totals(root.path(), &AtomicBool::new(false)).unwrap();
            assert_eq!((result.files, result.directories, result.bytes), (3, 2, 3));
            assert!(result.errors.is_empty());
            assert_eq!(calls(), [vec![uid; expected], vec![gid; expected]]);
        }
    });
}

#[test]
fn failed_and_missing_account_records_keep_numeric_fallback() {
    controlled(|| {
        LOOKUPS.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot.as_mut().unwrap();
            state.answers[0].insert(11, Ok(None));
            state.answers[0].insert(12, Err(nix::errno::Errno::EIO));
            state.answers[1].insert(21, Ok(None));
            state.answers[1].insert(22, Err(nix::errno::Errno::EIO));
        });
        assert_eq!(get_username(11), "11");
        assert_eq!(get_username(12), "12");
        assert_eq!(get_groupname(21), "21");
        assert_eq!(get_groupname(22), "22");
    });
}

#[test]
fn cache_keys_names_and_numeric_fallbacks_by_id_and_account_kind() {
    controlled(|| {
        LOOKUPS.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot.as_mut().unwrap();
            state.answers[0].extend([
                (11, Ok(Some("alice".into()))),
                (12, Ok(None)),
                (13, Err(nix::errno::Errno::EIO)),
            ]);
            state.answers[1].extend([
                (11, Ok(Some("staff".into()))),
                (12, Err(nix::errno::Errno::EIO)),
                (13, Ok(None)),
            ]);
        });
        let mut cache = Cache::default();
        for _ in 0..2 {
            for (id, names) in [(11, "alice:staff"), (12, "12:12"), (13, "13:13")] {
                let owner = cache.lookup(id, id);
                assert_eq!((owner.uid, owner.gid), (id, id));
                assert_eq!(owner.display(OwnerDisplay::Names), names);
                assert_eq!(owner.display(OwnerDisplay::Numeric), format!("{id}:{id}"));
            }
            let owner = cache.lookup(11, 13);
            assert_eq!(owner.display(OwnerDisplay::Names), "alice:13");
            assert_eq!((owner.uid, owner.gid), (11, 13));
        }
        assert_eq!(calls(), [vec![11, 12, 13], vec![11, 12, 13]]);
    });
}

#[test]
fn a_new_operation_retries_failed_resolution_without_changing_the_old_snapshot() {
    controlled(|| {
        LOOKUPS.with(|slot| {
            slot.borrow_mut().as_mut().unwrap().answers[0].insert(7, Err(nix::errno::Errno::EIO));
        });
        let mut first = Cache::default();
        assert_eq!(first.lookup(7, 8).display(OwnerDisplay::Names), "7:group");
        LOOKUPS.with(|slot| {
            slot.borrow_mut().as_mut().unwrap().answers[0].insert(7, Ok(Some("available".into())));
        });
        let next = Cache::default().lookup(7, 8);
        assert_eq!(next.display(OwnerDisplay::Names), "available:group");
        assert_eq!(first.lookup(7, 8).display(OwnerDisplay::Names), "7:group");
        assert_eq!(calls(), [vec![7, 7], vec![8, 8]]);
    });
}
