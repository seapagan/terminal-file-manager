use super::*;
use crate::owner::{OwnerDisplay, windows::Ownership};

fn alice() -> Ownership {
    Ownership::Sid {
        sid: "S-1-5-18".into(),
        name: Name::Resolved("Alice".into()),
    }
}

fn prime(runtime: &mut Runtime, app: &mut App, inserted: Instant) {
    runtime.synchronize(app);
    for pane in &mut app.panes {
        pane.entries[0].windows_owner = alice();
        pane.refilter();
    }
    let key = super::super::Key::new(&app.panes[0].entries[0]).unwrap();
    runtime.paths.insert(key, alice(), inserted);
    runtime
        .names
        .insert("S-1-5-18".into(), Name::Resolved("Alice".into()), inserted);
}

#[test]
fn manual_refresh_preserves_presentation_then_revalidates_same_identity() {
    let (root, mut app, geometry) = loaded(1);
    let gate = Release::new();
    let wait = gate.0.clone();
    let mut runtime = Runtime::using(
        move |_, _| {
            blocked(&wait);
            Ok("S-1-5-19".into())
        },
        |_| Name::Resolved("Bob".into()),
    );
    prime(&mut runtime, &mut app, Instant::now());
    let request = app.request_refresh(0).unwrap();
    let snapshot = fs::list(
        root.path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(app.complete_listing(0, request.generation, Ok(snapshot)));
    assert_eq!(
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names),
        "Alice"
    );
    app.request_metadata(0).unwrap();
    app.popup = true;
    runtime.poll(&mut app, &geometry);
    assert_eq!(
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names),
        "Alice"
    );
    assert!(!runtime.readers.active.is_empty());
    drop(gate);
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names) == "Bob"
            && app.details[0]
                .entry
                .as_ref()
                .unwrap()
                .owner_display(OwnerDisplay::Names)
                == "Bob"
    });
}

#[test]
fn expired_path_revalidates_known_owner_and_updates_sidebar() {
    let (_root, mut app, geometry) = loaded(1);
    let gate = Release::new();
    let wait = gate.0.clone();
    let mut runtime = Runtime::using(
        move |_, _| {
            blocked(&wait);
            Ok("S-1-5-19".into())
        },
        |_| Name::Resolved("Bob".into()),
    );
    prime(
        &mut runtime,
        &mut app,
        Instant::now() - Duration::from_secs(31),
    );
    app.request_metadata(0).unwrap();
    app.sidebar = true;
    runtime.poll(&mut app, &geometry);
    assert!(!runtime.readers.active.is_empty());
    assert_eq!(
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names),
        "Alice"
    );
    drop(gate);
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names) == "Bob"
            && app.details[0]
                .entry
                .as_ref()
                .unwrap()
                .owner_display(OwnerDisplay::Names)
                == "Bob"
    });
}

#[test]
fn expired_name_keeps_resolved_presentation_until_translation_completes() {
    let (_root, mut app, geometry) = loaded(1);
    let gate = Release::new();
    let wait = gate.0.clone();
    let mut runtime = Runtime::using(
        |_, _| Ok("S-1-5-18".into()),
        move |_| {
            blocked(&wait);
            Name::Resolved("Renamed".into())
        },
    );
    prime(&mut runtime, &mut app, Instant::now());
    runtime.names.insert(
        "S-1-5-18".into(),
        Name::Resolved("Alice".into()),
        Instant::now() - Duration::from_secs(301),
    );
    let key = super::super::Key::new(&app.panes[0].entries[0]).unwrap();
    runtime.paths.insert(
        key,
        Ownership::Sid {
            sid: "S-1-5-18".into(),
            name: Name::Pending,
        },
        Instant::now(),
    );
    runtime.poll(&mut app, &geometry);
    assert_eq!(
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names),
        "Alice"
    );
    assert_eq!(runtime.namers.active.len(), 1);
    drop(gate);
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[0].owner_display(OwnerDisplay::Names) == "Renamed"
    });
}

#[test]
fn refresh_never_carries_an_owner_to_replaced_or_unidentified_entries() {
    for object in [Some((123, 456)), None] {
        let (root, mut app, _) = loaded(1);
        app.panes[0].entries[0].windows_owner = alice();
        let request = app.request_refresh(0).unwrap();
        let mut snapshot = fs::list(
            root.path(),
            &fs::IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap();
        snapshot.entries[0].identity.as_mut().unwrap().object = object;
        assert!(app.complete_listing(0, request.generation, Ok(snapshot)));
        assert_eq!(app.panes[0].entries[0].windows_owner, Ownership::Pending);
    }
}
