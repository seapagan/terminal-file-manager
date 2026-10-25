//! Platform refresh policy feeds bounded same-directory listing intents.
mod native;
#[cfg(test)]
mod polling_tests;
mod schedule;
#[cfg(test)]
mod tests;
use crate::{app::App, operations::NoticeSeverity, workers::Pool};
use native::{Binding, DIRTY, ERROR, READY, Signals, Worker};
use schedule::Schedule;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

/// Coordinates native refresh hints; Windows uses explicit manual refresh only.
pub struct Runtime {
    slots: [Slot; 2],
    mode: Mode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Native,
    Manual,
}

fn interval(failed: bool) -> Duration {
    Duration::from_secs(if failed { 2 } else { 30 })
}

struct Slot {
    worker: Option<Worker>,
    signals: Arc<Signals>,
    path: Option<PathBuf>,
    context: u64,
    schedule: Schedule,
    sweep: Duration,
    failed: bool,
    urgent: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(if cfg!(windows) {
            Mode::Manual
        } else {
            Mode::Native
        })
    }
}

impl Runtime {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            slots: std::array::from_fn(|_| Slot {
                worker: match mode {
                    Mode::Native => Worker::new().ok(),
                    Mode::Manual => None,
                },
                signals: Arc::default(),
                path: None,
                context: 0,
                schedule: Schedule::default(),
                sweep: interval(false),
                failed: false,
                urgent: false,
            }),
        }
    }
}

impl Runtime {
    /// Consume bounded hints without waiting on filesystem calls.
    pub fn poll(&mut self, app: &mut App, pool: &mut Pool, now: Duration) -> bool {
        if self.mode == Mode::Manual {
            return false;
        }
        let mut changed = false;
        for (pane, slot) in self.slots.iter_mut().enumerate() {
            if app.panes[pane].context == 0 && app.panes[pane].listing {
                continue;
            }
            slot.rebind(&app.panes[pane], now);
            changed |= slot.consume(app, pane, now);
            if now >= slot.sweep {
                slot.urgent = true;
            }
            if slot.due(&app.panes[pane], now)
                && let Some(request) = app.request_reconciliation(pane)
            {
                slot.schedule.started(now);
                slot.urgent = false;
                slot.sweep = now + interval(slot.failed);
                pool.submit(request);
            }
        }
        changed
    }
}

impl Slot {
    fn due(&self, pane: &crate::app::Pane, now: Duration) -> bool {
        pane.pending.is_none() && !pane.listing && (self.urgent || self.schedule.due(now))
    }

    fn rebind(&mut self, pane: &crate::app::Pane, now: Duration) {
        if self.path.as_ref() == Some(&pane.path) && self.context == pane.context {
            return;
        }
        self.path = Some(pane.path.clone());
        self.context = pane.context;
        self.signals = Arc::default();
        self.schedule = Schedule::default();
        self.sweep = now + interval(false);
        self.urgent = false;
        if let Some(worker) = &self.worker {
            worker.bind(Binding {
                path: pane.path.clone(),
                signals: self.signals.clone(),
            });
        } else {
            self.signals.bits.store(ERROR | DIRTY, Ordering::Release);
        }
    }

    fn consume(&mut self, app: &mut App, pane: usize, now: Duration) -> bool {
        let bits = self.signals.bits.swap(0, Ordering::AcqRel);
        if bits & DIRTY != 0 {
            self.schedule.dirty(now);
        }
        let failed = if bits & ERROR != 0 {
            true
        } else if bits & READY != 0 {
            false
        } else {
            self.failed
        };
        let changed = self.failed != failed;
        self.failed = failed;
        if bits & READY != 0 || (changed && failed) {
            self.urgent = true;
        }
        if changed {
            let message = if failed {
                "Live filesystem notifications unavailable; periodically refreshing"
            } else {
                "Live filesystem notifications recovered"
            };
            app.operations.notify(
                format!("Pane {}: {message}", pane + 1),
                app.now,
                NoticeSeverity::Warning,
            );
        }
        changed
    }
}
