//! Six bounded I/O lanes: listing, metadata and totals independently for each pane.
use crate::{
    app::{Request, RequestKind},
    fs::{self, Entry, IgnoreContext, Snapshot, Totals},
};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

/// Background values are installed only by generation-checked model transitions.
pub enum Value {
    Listing(Result<Snapshot, String>),
    Metadata(Box<Entry>, Option<PathBuf>),
    Totals(Result<Totals, String>),
}

/// A terminal completion always releases its lane, including cancelled requests.
pub struct Completion {
    pub request: Request,
    pub value: Value,
}

#[derive(Default)]
struct Lane {
    running: Option<Arc<AtomicBool>>,
    pending: Option<Request>,
}

/// At most one running and one replaceable pending request per lane.
pub struct Pool {
    runtime: Option<tokio::runtime::Runtime>,
    lanes: [Lane; 6],
    sender: mpsc::SyncSender<Completion>,
    receiver: mpsc::Receiver<Completion>,
    context: IgnoreContext,
}

impl Pool {
    /// Create one runtime with a fixed bound on blocking filesystem calls.
    ///
    /// # Errors
    /// Returns the runtime's thread/resource allocation error.
    pub fn new(context: IgnoreContext) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(6)
            .build()?;
        let (sender, receiver) = mpsc::sync_channel(6);
        Ok(Self {
            runtime: Some(runtime),
            lanes: std::array::from_fn(|_| Lane::default()),
            sender,
            receiver,
            context,
        })
    }

    /// Replace queued work and cooperatively cancel the old read; never block input.
    pub fn submit(&mut self, request: Request) {
        let Some(index) = lane_index(&request) else {
            return;
        };
        let lane = &mut self.lanes[index];
        if let Some(cancelled) = &lane.running {
            cancelled.store(true, Ordering::Relaxed);
        }
        lane.pending = Some(request);
        self.pump();
    }

    /// Cancel a lane without claiming an in-flight kernel read was interrupted.
    pub fn cancel(&mut self, pane: usize, kind: RequestKind) {
        let request = Request {
            pane,
            kind,
            generation: 0,
            path: PathBuf::new(),
        };
        if let Some(index) = lane_index(&request) {
            self.lanes[index].pending = None;
            if let Some(cancelled) = &self.lanes[index].running {
                cancelled.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Nonblocking event collection on the UI loop; tests may use a bounded wait.
    pub fn next(&mut self, timeout: Duration) -> Option<Completion> {
        let completion = if timeout.is_zero() {
            self.receiver.try_recv().ok()
        } else {
            self.receiver.recv_timeout(timeout).ok()
        }?;
        if let Some(index) = lane_index(&completion.request) {
            self.lanes[index].running = None;
        }
        self.pump();
        Some(completion)
    }

    /// Current read concurrency for measurement and resource-budget assertions.
    pub fn active_count(&self) -> usize {
        self.lanes
            .iter()
            .filter(|lane| lane.running.is_some())
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
        for lane in &mut self.lanes {
            if lane.running.is_some() {
                continue;
            }
            let Some(request) = lane.pending.take() else {
                continue;
            };
            let cancelled = Arc::new(AtomicBool::new(false));
            lane.running = Some(cancelled.clone());
            let context = self.context.clone();
            let sender = self.sender.clone();
            self.runtime
                .as_ref()
                .expect("live runtime")
                .spawn_blocking(move || {
                    let value = execute(&request, &context, &cancelled);
                    let _ = sender.send(Completion { request, value });
                });
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        for lane in &mut self.lanes {
            if let Some(cancelled) = &lane.running {
                cancelled.store(true, Ordering::Relaxed);
            }
        }
        if let Some(runtime) = self.runtime.take() {
            // Blocking syscalls cannot be forcibly cancelled; all workers are read-only.
            runtime.shutdown_timeout(Duration::from_millis(100));
        }
    }
}

fn lane_index(request: &Request) -> Option<usize> {
    if request.pane >= 2 {
        return None;
    }
    let kind = match request.kind {
        RequestKind::Listing => 0,
        RequestKind::Metadata => 1,
        RequestKind::Totals => 2,
    };
    Some(kind * 2 + request.pane)
}

fn execute(request: &Request, context: &IgnoreContext, cancelled: &AtomicBool) -> Value {
    match request.kind {
        RequestKind::Listing => {
            Value::Listing(fs::list(&request.path, context, cancelled).map_err(|e| e.to_string()))
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
