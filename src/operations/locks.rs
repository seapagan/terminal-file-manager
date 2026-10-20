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
}

fn footprint(plan: &Plan) -> io::Result<Vec<Claim>> {
    let mut claims = Vec::new();
    for entry in &plan.operands {
        let identity = entry
            .identity
            .as_ref()
            .ok_or_else(|| invalid("Missing source identity"))?;
        claims.push(Claim {
            path: namespace(&entry.path)?,
            object: identity.object,
            write: plan.operation != Operation::Copy,
            unknown_aliases: unknown_aliases(entry),
        });
        if let Some(destination) = plan.target(entry) {
            claims.push(destination_claim(&destination)?);
        }
    }
    if plan.operands.is_empty()
        && let Some(destination) = &plan.destination
    {
        claims.push(destination_claim(destination)?);
    }
    claims.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(claims)
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
