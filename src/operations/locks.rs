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
}

/// One process-wide coordinator for all local mutation kinds, with two active jobs.
#[derive(Default)]
pub struct Coordinator {
    claims: BTreeMap<u64, Vec<Claim>>,
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
        if self.claims.len() >= 2 {
            return Ok(false);
        }
        let claims = footprint(plan)?;
        if self
            .claims
            .values()
            .flatten()
            .any(|held| claims.iter().any(|next| conflicts(held, next)))
        {
            return Ok(false);
        }
        self.claims.insert(job, claims);
        Ok(true)
    }

    /// Release only after acknowledged completion/cancellation or reconciliation.
    pub fn release(&mut self, job: u64) {
        self.claims.remove(&job);
    }

    /// Jobs holding claims, including unresolved outcomes.
    pub fn active(&self) -> usize {
        self.claims.len()
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
        });
    }
    if let Some(destination) = &plan.destination {
        let path = if matches!(plan.operation, Operation::Copy | Operation::Move) {
            std::fs::canonicalize(destination)?
        } else {
            namespace(destination)?
        };
        claims.push(Claim {
            path,
            object: None,
            write: true,
        });
    }
    claims.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(claims)
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
