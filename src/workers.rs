//! Bounded I/O lanes for listings, metadata and totals.
use crate::{
    app::{Request, RequestKind},
    fs::{self, Entry, Identity, IgnoreContext, Snapshot, Totals},
};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

const PANE_COUNT: usize = 2;
const LANE_COUNT: usize = 3 * PANE_COUNT;
const WORKER_COUNT: usize = LANE_COUNT + PANE_COUNT;

/// Background values are installed only by generation-checked model transitions.
pub enum Value {
    Listing(Result<Snapshot, String>),
    ParentListing(Result<Snapshot, String>, Option<Identity>),
    Metadata(Box<Entry>, Option<PathBuf>),
    Totals(Result<Totals, String>),
    Failed(String),
}

/// A terminal completion always releases its worker slot, including cancelled requests.
pub struct Completion {
    pub request: Request,
    pub value: Value,
}

struct Finished {
    lane: usize,
    slot: usize,
    completion: Completion,
}

#[derive(Default)]
struct Lane {
    running: [Option<Arc<AtomicBool>>; 2],
    pending: Option<Work>,
}

impl Lane {
    fn cancel(&mut self) {
        self.pending = None;
        for cancelled in self.running.iter().flatten() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
}

struct Work {
    request: Request,
    refresh: Option<(Snapshot, Vec<PathBuf>)>,
}
#[derive(Default)]
struct Metrics {
    full_scans: AtomicUsize,
    metadata_refreshes: AtomicUsize,
}
/// Executed listing work, rather than hint or scheduling counts.
#[derive(Clone, Copy, Debug)]
pub struct ListingMetrics {
    pub full_scans: usize,
    pub metadata_refreshes: usize,
}

/// Two listing slots and one metadata and totals slot each per pane.
/// Each lane holds at most one replaceable pending request.
pub struct Pool {
    runtime: Option<tokio::runtime::Runtime>,
    lanes: [Lane; LANE_COUNT],
    sender: mpsc::SyncSender<Finished>,
    receiver: mpsc::Receiver<Finished>,
    context: IgnoreContext,
    metrics: Arc<Metrics>,
    #[cfg(test)]
    pub(crate) before_execute: Option<BeforeExecute>,
}

#[cfg(test)]
type BeforeExecute = Arc<dyn Fn(&Request, &AtomicBool) + Send + Sync>;

impl Pool {
    /// Create one runtime with a fixed bound on blocking filesystem calls.
    ///
    /// # Errors
    /// Returns the runtime's thread/resource allocation error.
    pub fn new(context: IgnoreContext) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(WORKER_COUNT)
            .build()?;
        let (sender, receiver) = mpsc::sync_channel(WORKER_COUNT);
        Ok(Self {
            runtime: Some(runtime),
            lanes: std::array::from_fn(|_| Lane::default()),
            sender,
            receiver,
            context,
            metrics: Arc::default(),
            #[cfg(test)]
            before_execute: None,
        })
    }

    /// Replace queued work and cooperatively cancel the old read; never block input.
    pub fn submit(&mut self, request: Request) {
        self.submit_work(Work {
            request,
            refresh: None,
        });
    }
    pub(crate) fn submit_metadata_refresh(
        &mut self,
        request: Request,
        seed: Snapshot,
        paths: Vec<PathBuf>,
    ) {
        self.submit_work(Work {
            request,
            refresh: Some((seed, paths)),
        });
    }
    fn submit_work(&mut self, work: Work) {
        let Request { pane, kind, .. } = &work.request;
        let Some(index) = lane_index(*pane, kind) else {
            return;
        };
        let lane = &mut self.lanes[index];
        lane.cancel();
        lane.pending = Some(work);
        self.pump();
    }
    pub fn listing_metrics(&self) -> ListingMetrics {
        ListingMetrics {
            full_scans: self.metrics.full_scans.load(Ordering::Relaxed),
            metadata_refreshes: self.metrics.metadata_refreshes.load(Ordering::Relaxed),
        }
    }

    /// Cancel a lane without claiming an in-flight kernel read was interrupted.
    pub fn cancel(&mut self, pane: usize, kind: RequestKind) {
        if let Some(index) = lane_index(pane, &kind) {
            self.lanes[index].cancel();
        }
    }

    /// Nonblocking event collection on the UI loop; tests may use a bounded wait.
    pub fn next(&mut self, timeout: Duration) -> Option<Completion> {
        let finished = if timeout.is_zero() {
            self.receiver.try_recv().ok()
        } else {
            self.receiver.recv_timeout(timeout).ok()
        }?;
        self.lanes[finished.lane].running[finished.slot] = None;
        self.pump();
        Some(finished.completion)
    }

    /// Current read concurrency for measurement and resource-budget assertions.
    pub fn active_count(&self) -> usize {
        self.lanes
            .iter()
            .flat_map(|lane| lane.running.iter().flatten())
            .count()
    }

    /// Queued replaceable requests, always bounded independently of key count.
    pub fn pending_count(&self) -> usize {
        self.lanes
            .iter()
            .filter(|lane| lane.pending.is_some())
            .count()
    }

    fn pump(&mut self) {
        for (index, lane) in self.lanes.iter_mut().enumerate() {
            // Either listing kind can replace a scan still blocked in the kernel.
            let capacity = if index < PANE_COUNT { 2 } else { 1 };
            let Some(slot) = lane.running[..capacity].iter().position(Option::is_none) else {
                continue;
            };
            let Some(Work { request, refresh }) = lane.pending.take() else {
                continue;
            };
            let cancelled = Arc::new(AtomicBool::new(false));
            lane.running[slot] = Some(cancelled.clone());
            let context = self.context.clone();
            let metrics = self.metrics.clone();
            let sender = self.sender.clone();
            #[cfg(test)]
            let before_execute = self.before_execute.clone();
            self.runtime
                .as_ref()
                .expect("live runtime")
                .spawn(async move {
                    let work = request.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        #[cfg(test)]
                        if let Some(before_execute) = before_execute {
                            before_execute(&work, &cancelled);
                        }
                        execute_work(&work, &context, &cancelled, refresh, &metrics)
                    })
                    .await;
                    let value = result.unwrap_or_else(|error| {
                        Value::Failed(format!("{:?} worker failed: {error}", request.kind))
                    });
                    let _ = sender.send(Finished {
                        lane: index,
                        slot,
                        completion: Completion { request, value },
                    });
                });
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        for lane in &mut self.lanes {
            lane.cancel();
        }
        if let Some(runtime) = self.runtime.take() {
            // Blocking syscalls cannot be forcibly cancelled; all workers are read-only.
            runtime.shutdown_timeout(Duration::from_millis(100));
        }
    }
}

fn lane_index(pane: usize, kind: &RequestKind) -> Option<usize> {
    if pane >= PANE_COUNT {
        return None;
    }
    let kind = match kind {
        RequestKind::Listing | RequestKind::ParentListing(_) => 0,
        RequestKind::Metadata => 1,
        RequestKind::Totals => 2,
    };
    Some(kind * PANE_COUNT + pane)
}

fn execute_work(
    request: &Request,
    context: &IgnoreContext,
    cancelled: &AtomicBool,
    refresh: Option<(Snapshot, Vec<PathBuf>)>,
    metrics: &Metrics,
) -> Value {
    if let Some((seed, paths)) = refresh {
        match fs::refresh_metadata(seed, &paths, cancelled) {
            Ok(Some(snapshot)) => {
                metrics.metadata_refreshes.fetch_add(1, Ordering::Relaxed);
                return Value::Listing(Ok(snapshot));
            }
            Err(error) => return Value::Listing(Err(error.to_string())),
            Ok(None) => {}
        }
    }
    if matches!(
        request.kind,
        RequestKind::Listing | RequestKind::ParentListing(_)
    ) {
        metrics.full_scans.fetch_add(1, Ordering::Relaxed);
    }
    execute(request, context, cancelled)
}

fn execute(request: &Request, context: &IgnoreContext, cancelled: &AtomicBool) -> Value {
    match &request.kind {
        RequestKind::Listing => {
            Value::Listing(fs::list(&request.path, context, cancelled).map_err(|e| e.to_string()))
        }
        RequestKind::ParentListing(exited) => {
            execute_parent(request, exited.as_deref(), context, cancelled)
        }
        RequestKind::Metadata => Value::Metadata(
            Box::new(fs::inspect(&request.path)),
            std::fs::canonicalize(&request.path).ok(),
        ),
        RequestKind::Totals => {
            Value::Totals(fs::totals(&request.path, cancelled).map_err(|e| e.to_string()))
        }
    }
}

fn execute_parent(
    request: &Request,
    exited: Option<&std::path::Path>,
    context: &IgnoreContext,
    cancelled: &AtomicBool,
) -> Value {
    let identity = exited
        .filter(|_| !cancelled.load(Ordering::Relaxed))
        .and_then(|path| fs::inspect_identity(path).identity)
        .filter(|identity| identity.object.is_some());
    let result = fs::list(&request.path, context, cancelled).map_err(|error| error.to_string());
    Value::ParentListing(result, identity)
}

#[cfg(test)]
mod tests;
