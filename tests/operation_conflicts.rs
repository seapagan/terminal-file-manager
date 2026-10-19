use std::{fs, path::Path};
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute,
};

fn run(
    root: &Path,
    decide: impl FnMut(Event) -> Decision,
) -> terminal_file_manager::operations::Record {
    let plan = Plan::paths(
        Operation::Copy,
        vec![root.join("source")],
        Some(root.join("target")),
    )
    .unwrap();
    execute(&plan, Options::default(), &false.into(), decide)
}

#[test]
fn keep_both_reserves_an_unused_name_and_preserves_both_types() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "incoming").unwrap();
    fs::create_dir_all(root.path().join("target/source")).unwrap();
    fs::write(root.path().join("target/source/sentinel"), "existing").unwrap();
    fs::write(root.path().join("target/source (copy 1)"), "occupied").unwrap();
    let record = run(root.path(), |_| Decision::KeepBoth);
    assert_eq!(record.completed_files(), 1);
    assert_eq!(
        fs::read(root.path().join("target/source (copy 2)")).unwrap(),
        b"incoming"
    );
    assert_eq!(
        fs::read(root.path().join("target/source/sentinel")).unwrap(),
        b"existing"
    );
    assert_eq!(
        fs::read(root.path().join("target/source (copy 1)")).unwrap(),
        b"occupied"
    );
}

#[test]
fn apply_all_never_crosses_type_class_or_job() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source")).unwrap();
    fs::create_dir_all(root.path().join("target/source/c")).unwrap();
    for name in ["a", "b", "c"] {
        fs::write(root.path().join("source").join(name), "incoming").unwrap();
    }
    for name in ["a", "b"] {
        fs::write(root.path().join("target/source").join(name), "old").unwrap();
    }
    let mut file_prompts = 0;
    let mut type_prompts = 0;
    let record = run(root.path(), |event| {
        if let Event::Conflict { destination, .. } = event {
            if destination.may_recurse() {
                type_prompts += 1;
                return Decision::Skip;
            }
            file_prompts += 1;
            return Decision::All(Box::new(Decision::Replace));
        }
        Decision::Skip
    });
    assert_eq!((file_prompts, type_prompts), (1, 1));
    assert_eq!(record.completed_files(), 2);
    let mut prompted = false;
    run(root.path(), |event| {
        prompted |= matches!(event, Event::Conflict { .. });
        Decision::Skip
    });
    assert!(prompted);
}

#[test]
fn cancelling_at_a_conflict_stops_subsequent_operands() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    for name in ["a", "b"] {
        fs::write(root.path().join(name), "incoming").unwrap();
    }
    fs::write(root.path().join("target/a"), "previous").unwrap();
    let plan = Plan::paths(
        Operation::Move,
        vec![root.path().join("a"), root.path().join("b")],
        Some(root.path().join("target")),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| {
        Decision::Cancel
    });
    assert!(root.path().join("a").exists());
    assert!(root.path().join("b").exists());
    assert!(!root.path().join("target/b").exists());
    assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
}

#[test]
fn a_new_keep_both_collision_never_overwrites_the_external_object() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "incoming").unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    fs::write(root.path().join("target/source"), "old").unwrap();
    let collision = root.path().join("target/source (copy 1)");
    let record = run(root.path(), |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Verified,
                ..
            }
        ) {
            fs::write(&collision, "external").unwrap();
        }
        Decision::KeepBoth
    });
    assert_eq!(fs::read(collision).unwrap(), b"external");
    assert_eq!(fs::read(root.path().join("target/source")).unwrap(), b"old");
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
}

#[test]
fn staged_identity_replacement_is_quarantined_and_never_cleaned_as_owned() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "incoming").unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    let mut replacement = None;
    let record = run(root.path(), |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Verified,
                ..
            }
        ) {
            let directory = fs::read_dir(root.path().join("target"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let payload = directory.join("payload");
            fs::rename(&payload, directory.join("retained-original")).unwrap();
            fs::write(&payload, "external object").unwrap();
            replacement = Some(payload);
        }
        Decision::Skip
    });
    assert!(!root.path().join("target/source").exists());
    assert_eq!(fs::read(replacement.unwrap()).unwrap(), b"external object");
    assert_eq!(record.entries[0].outcome, Outcome::OutcomeUnknown);
    assert!(record.entries[0].detail.contains("tfm-stage"));
}
