use std::sync::atomic::{AtomicU8, Ordering};

pub(super) const DIRTY: u8 = 1;
pub(super) const READY: u8 = 2;
pub(super) const ERROR: u8 = 4;

#[derive(Default)]
pub(super) struct Signals {
    pub(super) bits: AtomicU8,
}

impl Signals {
    pub(super) fn publish(&self, bits: u8) {
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
