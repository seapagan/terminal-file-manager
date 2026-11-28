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
    hints: signals::Hints,
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
                hints: Default::default(),
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
            if app.panes[pane].is_remote() {
                continue;
            }
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
                slot.submit(request, &app.panes[pane], pool, now);
            }
        }
        changed
    }
}

impl Slot {
    fn submit(
        &mut self,
        request: crate::app::Request,
        pane: &crate::app::Pane,
        pool: &mut Pool,
        now: Duration,
    ) {
        let full = self.urgent || self.failed || self.hints.full;
        let hints = std::mem::take(&mut self.hints);
        self.schedule.started(now);
        self.urgent = false;
        if full {
            self.sweep = now
                + if self.failed {
                    (self.fallback.next() * 2).min(Duration::from_secs(30))
                } else {
                    Duration::from_secs(30)
                };
        }
        self.retry = None;
        if full || hints.children.is_empty() {
            pool.submit(request);
        } else {
            pool.submit_metadata_refresh(
                request,
                pane.cached_snapshot(),
                hints.children.into_iter().collect(),
            );
        }
    }

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
        if self.path.as_ref() == Some(pane.local_path()) && self.context == pane.context {
            return;
        }
        self.path = Some(pane.local_path().clone());
        self.context = pane.context;
        self.signals = Arc::default();
        self.schedule = Schedule::default();
        self.hints = Default::default();
        self.sweep = now + Duration::from_secs(30);
        self.urgent = false;
        self.fallback = Backoff::default();
        self.retry = None;
        self.retry_backoff = Backoff::default();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let Some(worker) = &self.worker {
            worker.bind(Binding {
                path: pane.local_path().clone(),
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
        if bits & DIRTY != 0 {
            self.hints.merge(self.signals.take_hints());
        }
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
            announce(app, pane, failed);
        }
        changed
    }
}

fn announce(app: &mut App, pane: usize, failed: bool) {
    let message = if failed {
        "Live filesystem notifications unavailable; periodically refreshing"
    } else {
        "Live filesystem notifications recovered"
    };
    app.notifications.notify(
        format!("Pane {}: {message}", pane + 1),
        app.now,
        if failed {
            NoticeSeverity::Warning
        } else {
            NoticeSeverity::Info
        },
    );
}

fn new_worker(mode: Mode) -> Option<NativeWorker> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if mode == Mode::Native {
        return Worker::new().ok();
    }
    let _ = mode;
    None
}
