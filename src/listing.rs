//! Deterministic pane sorting, independent of directory grouping.

use crate::fs::Entry;
use std::{cmp::Ordering, ffi::OsStr};

/// Sort order; missing metadata sorts last.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sort {
    #[default]
    Natural,
    Name,
    Extension,
    Size,
    Modified,
    Created,
    Kind,
    Version,
    Unsorted,
}

/// Directory grouping remains independent of reverse direction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Grouping {
    #[default]
    First,
    Last,
    Mixed,
}

/// One pane's listing policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct SortPolicy {
    pub sort: Sort,
    pub grouping: Grouping,
    pub reverse: bool,
}

impl SortPolicy {
    /// Stable sorting preserves filesystem order in Unsorted mode.
    pub fn apply(self, entries: &mut [Entry]) {
        entries.sort_by(|a, b| {
            let grouping = match self.grouping {
                Grouping::First => b.enterable.cmp(&a.enterable),
                Grouping::Last => a.enterable.cmp(&b.enterable),
                Grouping::Mixed => Ordering::Equal,
            };
            grouping.then_with(|| {
                let result = compare(a, b, self.sort);
                if self.reverse {
                    result.reverse()
                } else {
                    result
                }
            })
        });
    }
}

fn compare(a: &Entry, b: &Entry, sort: Sort) -> Ordering {
    let ordering = match sort {
        Sort::Unsorted => return Ordering::Equal,
        Sort::Natural => natural(a.name.as_encoded_bytes(), b.name.as_encoded_bytes()),
        Sort::Name => name(&a.name, &b.name),
        Sort::Extension => a.path.extension().cmp(&b.path.extension()),
        Sort::Size => b.size.cmp(&a.size),
        Sort::Modified => b.modified.cmp(&a.modified),
        Sort::Created => b.created.cmp(&a.created),
        Sort::Kind => a.kind.cmp(&b.kind),
        Sort::Version => a
            .name
            .to_str()
            .zip(b.name.to_str())
            .map_or_else(|| name(&a.name, &b.name), |(a, b)| vsort::compare(a, b)),
    };
    ordering.then_with(|| name(&a.name, &b.name))
}

fn name(a: &OsStr, b: &OsStr) -> Ordering {
    a.to_string_lossy()
        .to_lowercase()
        .cmp(&b.to_string_lossy().to_lowercase())
        .then_with(|| a.cmp(b))
}

fn natural(mut a: &[u8], mut b: &[u8]) -> Ordering {
    while let (Some(&left), Some(&right)) = (a.first(), b.first()) {
        if left.is_ascii_digit() && right.is_ascii_digit() {
            let alen = a.iter().take_while(|c| c.is_ascii_digit()).count();
            let blen = b.iter().take_while(|c| c.is_ascii_digit()).count();
            let an = trim_zeros(&a[..alen]);
            let bn = trim_zeros(&b[..blen]);
            let order = an.len().cmp(&bn.len()).then_with(|| an.cmp(bn));
            if order != Ordering::Equal {
                return order;
            }
            a = &a[alen..];
            b = &b[blen..];
        } else {
            let order = left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase());
            if order != Ordering::Equal {
                return order;
            }
            a = &a[1..];
            b = &b[1..];
        }
    }
    a.len().cmp(&b.len())
}

fn trim_zeros(bytes: &[u8]) -> &[u8] {
    &bytes[bytes.iter().take_while(|b| **b == b'0').count()..]
}
