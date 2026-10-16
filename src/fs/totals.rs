//! On-demand traversal uses a depth-bounded stack of open iterators, not a tree copy.

use super::{check_cancelled, inspect};
use std::{fs, io, path::Path, sync::atomic::AtomicBool};

/// Observed regular-file bytes and descendants. Errors mean an incomplete scan.
#[derive(Clone, Debug, Default)]
pub struct Totals {
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
    pub errors: Vec<String>,
}

/// Count descendants without following links, junctions or other reparse points.
///
/// # Errors
/// Returns Interrupted on cancellation or the explicit root's access error.
pub fn totals(path: &Path, cancelled: &AtomicBool) -> io::Result<Totals> {
    check_cancelled(cancelled)?;
    let mut stack = vec![fs::read_dir(path)?];
    let mut result = Totals::default();
    while let Some(directory) = stack.last_mut() {
        check_cancelled(cancelled)?;
        match directory.next() {
            None => {
                stack.pop();
            }
            Some(Err(error)) => record_error(&mut result, error.to_string()),
            Some(Ok(item)) => {
                let entry = inspect(&item.path());
                count_entry(&entry, &mut result);
                if entry.may_recurse() {
                    descend(&entry.path, &mut stack, &mut result);
                }
            }
        }
    }
    Ok(result)
}

fn descend(path: &Path, stack: &mut Vec<fs::ReadDir>, result: &mut Totals) {
    if stack.len() >= 128 {
        record_error(result, format!("Depth limit reached: {}", path.display()));
        return;
    }
    match fs::read_dir(path) {
        Ok(directory) => stack.push(directory),
        Err(error) => record_error(result, format!("{}: {error}", path.display())),
    }
}

fn count_entry(entry: &super::Entry, result: &mut Totals) {
    match entry.kind {
        super::EntryKind::Directory => result.directories += 1,
        super::EntryKind::File => {
            result.files += 1;
            result.bytes = result.bytes.saturating_add(entry.size.unwrap_or(0));
        }
        _ => {}
    }
    if let Some(error) = &entry.error {
        record_error(result, format!("{}: {error}", entry.path.display()));
    }
}

fn record_error(result: &mut Totals, error: String) {
    // Keep an explicit incomplete marker without unbounded repeated diagnostics.
    if result.errors.len() < 32 {
        result.errors.push(error);
    }
}
