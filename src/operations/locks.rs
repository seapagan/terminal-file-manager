use super::{Operation, Plan, plan::invalid};
use std::{
    collections::{BTreeMap, HashSet},
    io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct Claim {
    path: PathBuf,
    object: Option<(u64, u64)>,
    write: bool,
    unknown_aliases: bool,
    objects: HashSet<(u64, u64)>,
}

mod aliases;

/// One process-wide coordinator for all local mutation kinds, with two active jobs.
#[derive(Default)]
pub struct Coordinator {
    claims: BTreeMap<u64, (Vec<Claim>, bool)>,
}

impl Coordinator {
    /// Acquire every claim together, or acquire none. Run on a worker.
    ///
    /// # Errors
    /// Reports ambiguous/unavailable namespace or identity; never grants a weak lock.
    pub fn acquire(&mut self, job: u64, plan: &Plan) -> io::Result<bool> {
        if self.claims.contains_key(&job) {
            return Err(invalid("Job already holds claims"));
        }
        if self.active() >= 2 {
            return Ok(false);
        }
        let claims = footprint(plan)?;
        if self
            .claims
            .values()
            .flat_map(|(claims, _)| claims)
            .any(|held| claims.iter().any(|next| conflicts(held, next)))
        {
            return Ok(false);
        }
        self.claims.insert(job, (claims, true));
        Ok(true)
    }

    /// Release only after acknowledged completion/cancellation or reconciliation.
    pub fn release(&mut self, job: u64) {
        self.claims.remove(&job);
    }

    /// Active workers; quarantined claims retain conflicts without consuming a slot.
    pub fn active(&self) -> usize {
        self.claims.values().filter(|(_, active)| *active).count()
    }

    /// An acknowledged stopped worker releases its slot while unresolved claims remain held.
    pub fn quarantine(&mut self, job: u64) {
        if let Some((_, active)) = self.claims.get_mut(&job) {
            *active = false;
        }
    }

    pub(super) fn resume(&mut self, job: u64) -> io::Result<bool> {
        if self.active() >= 2 {
            return Ok(false);
        }
        let (_, active) = self
            .claims
            .get_mut(&job)
            .ok_or_else(|| invalid("Reconciliation has no retained claims"))?;
        if *active {
            return Err(invalid("Job is still active"));
        }
        *active = true;
        Ok(true)
    }

    /// Claim newly allocated private objects before payload writes, including native aliases.
    ///
    /// # Errors
    /// Refuses changed ownership evidence or a job without its original claims.
    pub fn own_stage(
        &mut self,
        job: u64,
        root: &crate::fs::Entry,
        payload: &crate::fs::Entry,
    ) -> io::Result<bool> {
        super::safety::revalidate(root, false).map_err(|error| io::Error::other(error.detail))?;
        super::safety::revalidate(payload, true).map_err(|error| io::Error::other(error.detail))?;
        let additions = vec![stage_claim(root)?, stage_claim(payload)?];
        if self
            .claims
            .iter()
            .filter(|(id, _)| **id != job)
            .flat_map(|(_, (claims, _))| claims)
            .any(|held| additions.iter().any(|next| conflicts(held, next)))
        {
            return Ok(false);
        }
        let (claims, _) = self
            .claims
            .get_mut(&job)
            .ok_or_else(|| invalid("Stage has no owning job"))?;
        claims.extend(additions);
        Ok(true)
    }
}

fn stage_claim(entry: &crate::fs::Entry) -> io::Result<Claim> {
    let mut claim = source_claim(entry, Operation::Delete, None)?;
    claim
        .object
        .ok_or_else(|| invalid("Private stage native identity unavailable"))?;
    claim.unknown_aliases = false;
    Ok(claim)
}

fn footprint(plan: &Plan) -> io::Result<Vec<Claim>> {
    let mut claims = Vec::new();
    #[cfg(target_os = "linux")]
    if plan.operation == Operation::Trash {
        for entry in &plan.operands {
            for root in super::trash::claim_roots(entry)? {
                claims.push(trash_claim(&root)?);
            }
        }
    }
    if let Some(staging_namespace) = staging_claim(plan)? {
        claims.push(staging_namespace);
    }
    operand_claims(plan, &mut claims)?;
    claims.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(claims)
}

fn operand_claims(plan: &Plan, claims: &mut Vec<Claim>) -> io::Result<()> {
    for entry in &plan.operands {
        claims.push(source_claim(entry, plan.operation, plan.preview.as_ref())?);
        if let Some(destination) = plan.target(entry) {
            claims.push(destination_claim(&destination, true)?);
        }
    }
    if plan.operands.is_empty()
        && let Some(destination) = &plan.destination
    {
        claims.push(destination_claim(destination, true)?);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn trash_claim(root: &Path) -> io::Result<Claim> {
    Ok(Claim {
        path: future_namespace(root)?,
        object: crate::fs::inspect(root)
            .identity
            .and_then(|identity| identity.object),
        write: true,
        unknown_aliases: false,
        objects: HashSet::new(),
    })
}

#[cfg(target_os = "linux")]
fn future_namespace(path: &Path) -> io::Result<PathBuf> {
    let mut ancestor = std::path::absolute(path)?;
    let mut missing = Vec::new();
    loop {
        match std::fs::canonicalize(&ancestor) {
            Ok(mut resolved) => {
                for name in missing.into_iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| invalid("Trash namespace unavailable"))?
                        .to_owned(),
                );
                ancestor.pop();
            }
            Err(error) => return Err(error),
        }
    }
}

fn source_claim(
    entry: &crate::fs::Entry,
    operation: Operation,
    preview: Option<&super::Preview>,
) -> io::Result<Claim> {
    let identity = entry
        .identity
        .as_ref()
        .ok_or_else(|| invalid("Missing source identity"))?;
    // Destructive workers refuse descendants outside this immutable approval before mutation.
    let objects = match preview.filter(|_| {
        entry.may_recurse() && matches!(operation, Operation::Delete | Operation::Trash)
    }) {
        Some(preview) => preview.claim_objects(entry),
        None => aliases::capture(entry),
    };
    Ok(Claim {
        path: namespace(&entry.path)?,
        object: identity.object,
        write: operation != Operation::Copy,
        unknown_aliases: objects.is_err(),
        objects: objects.unwrap_or_default(),
    })
}

fn staging_claim(plan: &Plan) -> io::Result<Option<Claim>> {
    if !matches!(plan.operation, Operation::Copy | Operation::Move) {
        return Ok(None);
    }
    let Some(directory) = &plan.destination else {
        return Ok(None);
    };
    // Only private staging names are mutated here, not arbitrary existing descendants.
    destination_claim(directory, false).map(Some)
}

fn destination_claim(path: &Path, recursive: bool) -> io::Result<Claim> {
    let entry = match std::fs::symlink_metadata(path) {
        Ok(_) => Some(crate::fs::inspect(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let objects = entry
        .as_ref()
        .filter(|_| recursive)
        .map(aliases::capture)
        .transpose();
    let path = namespace(path)?;
    // An absent target belongs to its native parent, including through bind/junction aliases.
    let owner = entry.unwrap_or_else(|| crate::fs::inspect(path.parent().unwrap_or(&path)));
    let object = owner.identity.and_then(|id| id.object);
    Ok(Claim {
        path,
        object,
        write: true,
        unknown_aliases: objects.is_err() || object.is_none(),
        objects: objects.unwrap_or_default().unwrap_or_default(),
    })
}

pub(super) fn namespace(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| invalid("Filesystem root cannot be mutated"))?;
    let name = absolute
        .file_name()
        .ok_or_else(|| invalid("Missing entry name"))?;
    Ok(std::fs::canonicalize(parent)?.join(name))
}

fn conflicts(a: &Claim, b: &Claim) -> bool {
    if !a.write && !b.write {
        return false;
    }
    // Incomplete, changed or mounted inventories cannot establish independent ownership.
    if a.unknown_aliases || b.unknown_aliases {
        return true;
    }
    if object_conflicts(a, b) {
        return true;
    }
    #[cfg(windows)]
    {
        windows_overlap(&a.path, &b.path)
    }
    #[cfg(not(windows))]
    {
        a.path.starts_with(&b.path) || b.path.starts_with(&a.path)
    }
}

fn object_conflicts(a: &Claim, b: &Claim) -> bool {
    a.object
        .into_iter()
        .chain(a.objects.iter().copied())
        .any(|object| b.object == Some(object) || b.objects.contains(&object))
}

#[cfg(windows)]
fn windows_overlap(a: &Path, b: &Path) -> bool {
    let normalized = |path: &Path| {
        path.to_str()
            .filter(|text| text.is_ascii())
            .map(|text| PathBuf::from(text.to_ascii_lowercase()))
    };
    match (normalized(a), normalized(b)) {
        (Some(a), Some(b)) => a.starts_with(&b) || b.starts_with(&a),
        // Unknown native case equivalence conservatively serializes the namespace.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_future_destination_conflicts_with_its_native_parent_through_an_opaque_alias() {
        let root = tempfile::tempdir().unwrap();
        let parent = crate::fs::inspect(root.path());
        let source = source_claim(&parent, Operation::Delete, None).unwrap();
        let mut target = destination_claim(&root.path().join("new"), true).unwrap();
        // Bind aliases can preserve native identity without sharing a lexical prefix.
        target.path = PathBuf::from("opaque-native-alias/new");
        assert!(conflicts(&source, &target));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
