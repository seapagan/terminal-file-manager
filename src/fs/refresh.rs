//! Precise known-child hints avoid enumeration; uncertainty falls back to a full listing.
use super::*;
use std::collections::HashMap;

fn same_object(old: &Entry, fresh: &Entry) -> bool {
    old.identity
        .as_ref()
        .zip(fresh.identity.as_ref())
        .is_some_and(|(a, b)| a.object.is_some() && a.object == b.object && a.created == b.created)
        && fresh.error.is_none()
}

pub(crate) fn refresh_metadata(
    mut seed: Snapshot,
    paths: &[PathBuf],
    cancelled: &AtomicBool,
) -> io::Result<Option<Snapshot>> {
    if !seed.errors.is_empty() {
        return Ok(None);
    }
    let mut owners = crate::owner::Cache::default();
    let Some(before) = directory_identity(&seed.path, &mut owners) else {
        return Ok(None);
    };
    let lexical = inspect_no_follow(&seed.path, None);
    if !seed_matches(&seed, &before, &lexical) {
        return Ok(None);
    }
    if !update_known(&mut seed, paths, cancelled, &mut owners)? {
        return Ok(None);
    }
    check_cancelled(cancelled)?;
    let after = directory_identity(&seed.path, &mut owners);
    if after
        .as_ref()
        .is_none_or(|new| new.identity != before.identity)
        || inspect_no_follow(&seed.path, None).identity != lexical.identity
    {
        return Ok(None);
    }
    seed.directory = after.map(Box::new);
    Ok(Some(seed))
}

fn update_known(
    seed: &mut Snapshot,
    paths: &[PathBuf],
    cancelled: &AtomicBool,
    owners: &mut crate::owner::Cache,
) -> io::Result<bool> {
    let indices: HashMap<_, _> = seed
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.path.clone(), i))
        .collect();
    for path in paths {
        check_cancelled(cancelled)?;
        let Some(&index) = indices.get(path) else {
            return Ok(false);
        };
        let old = &seed.entries[index];
        let mut fresh = inspect_with_cache(path, owners);
        if !same_object(old, &fresh) {
            return Ok(false);
        }
        fresh.ignored = old.ignored;
        seed.entries[index] = fresh;
    }
    Ok(true)
}

fn seed_matches(seed: &Snapshot, before: &Entry, lexical: &Entry) -> bool {
    seed.directory
        .as_ref()
        .is_some_and(|old| old.identity == before.identity)
        && seed
            .entry_identity
            .as_ref()
            .zip(lexical.identity.as_ref())
            .is_some_and(|(a, b)| {
                a.object.is_some() && a.object == b.object && a.created == b.created
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seed(path: &Path) -> Snapshot {
        list(path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()
    }
    #[test]
    fn precise_refresh_updates_known_metadata_and_preserves_ignore_evidence() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("known");
        fs::write(&path, "old").unwrap();
        let mut old = seed(root.path());
        old.entries[0].ignored = Some(true);
        fs::write(&path, "new-length").unwrap();
        let refreshed = refresh_metadata(old, &[path], &AtomicBool::new(false))
            .unwrap()
            .unwrap();
        assert_eq!(refreshed.entries[0].size, Some(10));
        assert_eq!(refreshed.entries[0].ignored, Some(true));
    }
    #[test]
    fn stale_directory_and_unknown_or_replaced_children_require_full_reconciliation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("known");
        fs::write(&path, "old").unwrap();
        let mut old = seed(root.path());
        let missing = root.path().join("unknown");
        assert!(
            refresh_metadata(
                old.clone(),
                std::slice::from_ref(&missing),
                &AtomicBool::new(false)
            )
            .unwrap()
            .is_none()
        );
        fs::write(&missing, "new").unwrap();
        // NTFS need not publish a new directory timestamp before this query.
        // Exercise the stale-stamp guard without depending on timestamp timing.
        let identity = old.directory.as_mut().unwrap().identity.as_mut().unwrap();
        identity.modified = match identity.modified {
            Some(_) => None,
            None => Some(std::time::SystemTime::UNIX_EPOCH),
        };
        assert!(
            refresh_metadata(old, std::slice::from_ref(&path), &AtomicBool::new(false))
                .unwrap()
                .is_none()
        );
        let old = seed(root.path());
        fs::rename(&path, root.path().join("retained")).unwrap();
        fs::write(&path, "replacement").unwrap();
        assert!(
            refresh_metadata(old, &[path], &AtomicBool::new(false))
                .unwrap()
                .is_none()
        );
    }
}
