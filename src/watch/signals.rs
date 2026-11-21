use std::sync::atomic::{AtomicU8, Ordering};

pub(super) const DIRTY: u8 = 1;
pub(super) const READY: u8 = 2;
pub(super) const ERROR: u8 = 4;

#[derive(Default)]
pub(super) struct Signals {
    pub(super) bits: AtomicU8,
    hints: std::sync::Mutex<Hints>,
}

impl Signals {
    pub(super) fn publish(&self, bits: u8) {
        if bits & DIRTY != 0 {
            let mut hints = self.hints.lock().unwrap_or_else(|e| e.into_inner());
            hints.full = true;
            hints.children.clear();
        }
        if bits & (READY | ERROR) == 0 {
            self.bits.fetch_or(bits, Ordering::Release);
        } else {
            let mut previous = self.bits.load(Ordering::Acquire);
            loop {
                let next = (previous & !(READY | ERROR)) | bits;
                match self.bits.compare_exchange(
                    previous,
                    next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(current) => previous = current,
                }
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Hints {
    pub full: bool,
    pub children: std::collections::BTreeSet<std::path::PathBuf>,
}
impl Hints {
    pub fn merge(&mut self, other: Self) {
        self.full |= other.full;
        if !self.full {
            self.children.extend(other.children);
        }
        if self.children.len() > 128 {
            self.full = true;
        }
        if self.full {
            self.children.clear();
        }
    }
}
impl Signals {
    pub fn children(&self, paths: impl IntoIterator<Item = std::path::PathBuf>) {
        let mut hints = self.hints.lock().unwrap_or_else(|e| e.into_inner());
        for path in paths {
            if hints.full {
                break;
            }
            hints.children.insert(path);
            if hints.children.len() > 128 {
                hints.full = true;
                hints.children.clear();
            }
        }
        self.bits.fetch_or(DIRTY, Ordering::Release);
    }
    pub fn take_hints(&self) -> Hints {
        std::mem::take(&mut *self.hints.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precise_paths_are_bounded_and_full_invalidation_dominates() {
        let signals = Signals::default();
        signals.children(["known".into(), "known".into()]);
        assert_eq!(signals.take_hints().children.len(), 1);
        signals.publish(DIRTY);
        signals.children(["another".into()]);
        let hints = signals.take_hints();
        assert!(hints.full && hints.children.is_empty());
        signals.children((0..129).map(|i| format!("item-{i}").into()));
        let hints = signals.take_hints();
        assert!(hints.full && hints.children.is_empty());
    }
}
