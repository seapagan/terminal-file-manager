use super::{Operation, Plan, plan::invalid};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct Claim {
    path: PathBuf,
    object: Option<(u64, u64)>,
    write: bool,
    unknown_aliases: bool,
}

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
        let mut additions = vec![
            source_claim(root, Operation::Delete)?,
            source_claim(payload, Operation::Delete)?,
        ];
        for claim in &mut additions {
            claim.unknown_aliases = false;
        }
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

fn footprint(plan: &Plan) -> io::Result<Vec<Claim>> {
    let mut claims = Vec::new();
    #[cfg(target_os = "linux")]
    if plan.operation == Operation::Trash {
        claims.push(trash_claim()?);
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
        claims.push(source_claim(entry, plan.operation)?);
        if let Some(destination) = plan.target(entry) {
            claims.push(destination_claim(&destination)?);
        }
    }
    if plan.operands.is_empty()
        && let Some(destination) = &plan.destination
    {
        claims.push(destination_claim(destination)?);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn trash_claim() -> io::Result<Claim> {
    let root = super::trash::data_root()?.join("Trash");
    Ok(Claim {
        path: future_namespace(&root)?,
        object: crate::fs::inspect(&root)
            .identity
            .and_then(|identity| identity.object),
        write: true,
        unknown_aliases: false,
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

fn source_claim(entry: &crate::fs::Entry, operation: Operation) -> io::Result<Claim> {
    let identity = entry
        .identity
        .as_ref()
        .ok_or_else(|| invalid("Missing source identity"))?;
    Ok(Claim {
        path: namespace(&entry.path)?,
        object: identity.object,
        write: operation != Operation::Copy,
        unknown_aliases: unknown_aliases(entry),
    })
}

fn staging_claim(plan: &Plan) -> io::Result<Option<Claim>> {
    if !matches!(plan.operation, Operation::Copy | Operation::Move) {
        return Ok(None);
    }
    let Some(directory) = &plan.destination else {
        return Ok(None);
    };
    let mut claim = destination_claim(directory)?;
    // Only private staging names are mutated here, not arbitrary existing descendants.
    claim.unknown_aliases = false;
    Ok(Some(claim))
}

fn destination_claim(path: &Path) -> io::Result<Claim> {
    let entry = match std::fs::symlink_metadata(path) {
        Ok(_) => Some(crate::fs::inspect(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    Ok(Claim {
        path: namespace(path)?,
        object: entry
            .as_ref()
            .and_then(|entry| entry.identity.as_ref())
            .and_then(|id| id.object),
        write: true,
        unknown_aliases: entry.as_ref().is_some_and(unknown_aliases),
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
    // Descendant hard-link equivalence is unknown without a full namespace scan.
    if (a.unknown_aliases && b.object.is_some()) || (b.unknown_aliases && a.object.is_some()) {
        return true;
    }
    if a.object.is_some() && a.object == b.object {
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

fn unknown_aliases(entry: &crate::fs::Entry) -> bool {
    entry.may_recurse()
        && std::fs::read_dir(&entry.path)
            .map(|mut children| children.next().is_some())
            .unwrap_or(true)
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
