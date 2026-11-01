//! Platform refresh policy feeds bounded same-directory listing intents.
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod native;
#[cfg(test)]
mod polling_tests;
#[cfg(test)]
mod recovery_tests;
mod schedule;
mod signals;
#[cfg(test)]
mod tests;
use crate::{app::App, operations::NoticeSeverity, workers::Pool};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use native::{Binding, Worker};
use signals::{DIRTY, ERROR, READY, Signals};

#[cfg(any(target_os = "linux", target_os = "macos"))]
type NativeWorker = Worker;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
type NativeWorker = ();
use schedule::{Backoff, Schedule};
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

struct Slot {
    worker: Option<NativeWorker>,
    signals: Arc<Signals>,
    path: Option<PathBuf>,
    context: u64,
    schedule: Schedule,
    sweep: Duration,
    failed: bool,
    urgent: bool,
    fallback: Backoff,
    retry: Option<Duration>,
    retry_backoff: Backoff,
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
                worker: new_worker(mode),
                signals: Arc::default(),
                path: None,
                context: 0,
                schedule: Schedule::default(),
                sweep: Duration::from_secs(30),
                failed: false,
                urgent: false,
                fallback: Backoff::default(),
                retry: None,
                retry_backoff: Backoff::default(),
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
                slot.sweep = now
                    + if slot.failed {
                        (slot.fallback.next() * 2).min(Duration::from_secs(30))
                    } else {
                        Duration::from_secs(30)
                    };
                slot.retry = None;
                pool.submit(request);
            }
        }
        changed
    }
}

impl Slot {
    fn due(&mut self, pane: &crate::app::Pane, now: Duration) -> bool {
        if pane.pending.is_some() || pane.listing {
            return false;
        }
        if pane.error.is_some() {
            let deadline = self
                .retry
                .get_or_insert_with(|| now + self.retry_backoff.next());
            return now >= *deadline;
        }
        self.retry = None;
        self.retry_backoff = Backoff::default();
        self.urgent || self.schedule.due(now)
    }

    fn rebind(&mut self, pane: &crate::app::Pane, now: Duration) {
        if self.path.as_ref() == Some(&pane.path) && self.context == pane.context {
            return;
        }
        self.path = Some(pane.path.clone());
        self.context = pane.context;
        self.signals = Arc::default();
        self.schedule = Schedule::default();
        self.sweep = now + Duration::from_secs(30);
        self.urgent = false;
        self.fallback = Backoff::default();
        self.retry = None;
        self.retry_backoff = Backoff::default();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let Some(worker) = &self.worker {
            worker.bind(Binding {
                path: pane.path.clone(),
                signals: self.signals.clone(),
                failed: Arc::default(),
            });
        }
        if self.worker.is_none() {
            self.signals.publish(ERROR | DIRTY);
        }
    }

    fn consume(&mut self, app: &mut App, pane: usize, now: Duration) -> bool {
        let bits = self.signals.bits.swap(0, Ordering::AcqRel);
        // Repeated backend failures are covered by fallback, not fresh dirty work.
        if bits & (DIRTY | ERROR) == DIRTY {
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
        if bits & READY != 0 {
            self.fallback = Backoff::default();
        }
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

fn new_worker(mode: Mode) -> Option<NativeWorker> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if mode == Mode::Native {
        return Worker::new().ok();
    }
    let _ = mode;
    None
}
