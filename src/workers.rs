//! Bounded I/O lanes for listings, parent identity lookups, metadata and totals.
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

use tokio::sync::oneshot;

const PANE_COUNT: usize = 2;
const LANE_COUNT: usize = 4 * PANE_COUNT;
const WORKER_COUNT: usize = LANE_COUNT + PANE_COUNT;

/// Background values are installed only by generation-checked model transitions.
pub enum Value {
    Listing(Result<Snapshot, String>),
    SftpListing(Result<Vec<Entry>, String>),
    SftpFile,
    Cancelled,
    ParentIdentity(Option<Identity>),
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
    running: [Option<Cancellation>; 2],
    pending: Option<Work>,
}

enum Cancellation {
    Native(Arc<AtomicBool>),
    Sftp(Option<oneshot::Sender<()>>),
}

impl Cancellation {
    fn cancel(&mut self) {
        match self {
            Self::Native(flag) => flag.store(true, Ordering::Relaxed),
            Self::Sftp(signal) => {
                if let Some(signal) = signal.take() {
                    let _ = signal.send(());
                }
            }
        }
    }
}

impl Lane {
    fn cancel(&mut self) {
        self.pending = None;
        for cancelled in self.running.iter_mut().flatten() {
            cancelled.cancel();
        }
    }

    fn take_work(&mut self) -> Option<(usize, Work)> {
        let capacity = self.pending.as_ref()?.request.kind.worker_capacity();
        let slot = self.running[..capacity].iter().position(Option::is_none)?;
        Some((slot, self.pending.take()?))
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

/// Two listing slots, plus one slot each for parent identity, metadata and totals per pane.
/// Each lane holds at most one replaceable pending request.
pub struct Pool {
    runtime: Option<tokio::runtime::Runtime>,
    lanes: [Lane; LANE_COUNT],
    sender: mpsc::SyncSender<Finished>,
    receiver: mpsc::Receiver<Finished>,
    context: IgnoreContext,
    metrics: Arc<Metrics>,
    sftp: Option<Arc<crate::ssh::Session>>,
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
            .enable_time()
            .build()?;
        let (sender, receiver) = mpsc::sync_channel(WORKER_COUNT);
        Ok(Self {
            runtime: Some(runtime),
            lanes: std::array::from_fn(|_| Lane::default()),
            sender,
            receiver,
            context,
            metrics: Arc::default(),
            sftp: None,
            #[cfg(test)]
            before_execute: None,
        })
    }

    /// Attach the authenticated SFTP session for remote listing requests.
    pub fn attach_sftp(&mut self, session: Arc<crate::ssh::Session>) {
        self.sftp = Some(session);
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
        if kind.is_listing()
            && let Some(lookup_index) = lane_index(*pane, &RequestKind::ParentIdentity)
        {
            let lookup = &mut self.lanes[lookup_index];
            lookup.cancel();
            if let RequestKind::ParentListing(Some(path)) = kind {
                lookup.pending = Some(Work {
                    request: Request {
                        pane: *pane,
                        generation: work.request.generation,
                        path: Some(path.clone()),
                        kind: RequestKind::ParentIdentity,
                    },
                    refresh: None,
                });
            }
        }
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
            let Some((slot, Work { request, refresh })) = lane.take_work() else {
                continue;
            };
            let runtime = self.runtime.as_ref().expect("live runtime");
            let sender = self.sender.clone();
            if let Some(path) = request.kind.sftp_path().cloned() {
                let (signal, cancelled) = oneshot::channel();
                lane.running[slot] = Some(Cancellation::Sftp(Some(signal)));
                let sftp = self.sftp.clone();
                runtime.spawn(async move {
                    let activate = matches!(request.kind, RequestKind::SftpActivate(_));
                    let value = read_sftp(sftp, &path, activate, cancelled).await;
                    let _ = sender.send(finished(index, slot, request, value));
                });
            } else {
                let cancelled = Arc::new(AtomicBool::new(false));
                lane.running[slot] = Some(Cancellation::Native(cancelled.clone()));
                let context = self.context.clone();
                let metrics = self.metrics.clone();
                #[cfg(test)]
                let before_execute = self.before_execute.clone();
                runtime.spawn(async move {
                    let work = request.clone();
                    let value = tokio::task::spawn_blocking(move || {
                        #[cfg(test)]
                        if let Some(before_execute) = before_execute {
                            before_execute(&work, &cancelled);
                        }
                        execute_work(&work, &context, &cancelled, refresh, &metrics)
                    })
                    .await
                    .unwrap_or_else(|error| {
                        Value::Failed(format!(
                            "{} worker failed: {error}",
                            request.kind.worker_name()
                        ))
                    });
                    let _ = sender.send(finished(index, slot, request, value));
                });
            }
        }
    }
}

fn finished(lane: usize, slot: usize, request: Request, value: Value) -> Finished {
    Finished {
        lane,
        slot,
        completion: Completion { request, value },
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
        RequestKind::Listing
        | RequestKind::SftpListing(_)
        | RequestKind::SftpActivate(_)
        | RequestKind::ParentListing(_) => 0,
        RequestKind::Metadata => 1,
        RequestKind::Totals => 2,
        RequestKind::ParentIdentity => 3,
    };
    Some(kind * PANE_COUNT + pane)
}

impl RequestKind {
    fn is_listing(&self) -> bool {
        matches!(
            self,
            Self::Listing | Self::SftpListing(_) | Self::SftpActivate(_) | Self::ParentListing(_)
        )
    }

    fn worker_capacity(&self) -> usize {
        if self.is_listing() { 2 } else { 1 }
    }

    fn worker_name(&self) -> &'static str {
        match self {
            Self::Listing | Self::SftpListing(_) | Self::SftpActivate(_) => "Directory listing",
            Self::ParentListing(_) => "Parent listing",
            Self::ParentIdentity => "Parent identity lookup",
            Self::Metadata => "Metadata",
            Self::Totals => "Directory totals",
        }
    }
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
    if request.kind.is_listing() {
        metrics.full_scans.fetch_add(1, Ordering::Relaxed);
    }
    execute(request, context, cancelled)
}

fn execute(request: &Request, context: &IgnoreContext, cancelled: &AtomicBool) -> Value {
    let Some(path) = request.path.as_deref() else {
        return Value::Failed("Native worker request has no native path".into());
    };
    match &request.kind {
        RequestKind::SftpListing(_) | RequestKind::SftpActivate(_) => {
            unreachable!("SFTP requests execute asynchronously")
        }
        RequestKind::Listing | RequestKind::ParentListing(_) => {
            Value::Listing(fs::list(path, context, cancelled).map_err(|e| e.to_string()))
        }
        RequestKind::ParentIdentity => Value::ParentIdentity(
            fs::check_cancelled(cancelled)
                .ok()
                .and_then(|()| fs::inspect_identity(path).identity)
                .filter(|identity| identity.object.is_some()),
        ),
        RequestKind::Metadata => Value::Metadata(
            Box::new(fs::inspect(path)),
            std::fs::canonicalize(path).ok(),
        ),
        RequestKind::Totals => {
            Value::Totals(fs::totals(path, cancelled).map_err(|e| e.to_string()))
        }
    }
}

async fn read_sftp(
    session: Option<Arc<crate::ssh::Session>>,
    path: &russh_sftp::protocol::SftpString,
    activate: bool,
    cancelled: oneshot::Receiver<()>,
) -> Value {
    let Some(session) = session else {
        return Value::SftpListing(Err("SSH session is unavailable".into()));
    };
    let listing = session
        .browse_sftp(path.as_bytes(), activate, cancelled)
        .await;
    drop(session);
    match listing {
        Ok(Some(entries)) => Value::SftpListing(Ok(crate::app::remote_entries(entries))),
        Ok(None) => Value::SftpFile,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Value::Cancelled,
        Err(error) => Value::SftpListing(Err(error.to_string())),
    }
}

#[cfg(test)]
mod tests;
