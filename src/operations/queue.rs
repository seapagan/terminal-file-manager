use super::{Coordinator, Decision, EntryResult, Event, Options, Outcome, Plan, Record};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Reliable lifecycle/conflict events; replaceable progress is coalesced per job.
pub enum Update {
    Preview {
        id: u64,
        plan: Box<Plan>,
    },
    Started {
        id: u64,
    },
    Progress {
        id: u64,
        event: Event,
    },
    Conflict {
        id: u64,
        event: Event,
        reply: mpsc::SyncSender<Decision>,
    },
    Finished {
        id: u64,
        record: Record,
    },
}

struct Submission {
    id: u64,
    plan: Plan,
    options: Options,
    cancel: Arc<AtomicBool>,
    preview: bool,
    reconciliation: Option<Record>,
}

impl Submission {
    fn failed(&self, outcome: Outcome, detail: &str) -> Record {
        let Some(mut record) = self.reconciliation.clone() else {
            return unsuccessful(&self.plan, outcome, detail);
        };
        for entry in &mut record.entries {
            if entry.outcome == Outcome::OutcomeUnknown {
                entry.detail = format!("{}; {detail}", entry.detail);
            }
        }
        record
    }
}

struct Running {
    handle: JoinHandle<()>,
    progress: Arc<Mutex<Option<Event>>>,
}

enum Message {
    Update(Update),
    Claim {
        id: u64,
        root: Box<crate::fs::Entry>,
        payload: Box<crate::fs::Entry>,
        accepted: mpsc::SyncSender<bool>,
    },
}

/// Bounded local scheduler. Drop cancels and joins actual writers before returning.
pub struct Queue {
    sender: mpsc::SyncSender<Submission>,
    receiver: mpsc::Receiver<Update>,
    stop: Arc<AtomicBool>,
    tokens: HashMap<u64, Arc<AtomicBool>>,
    handle: Option<JoinHandle<()>>,
    next_id: u64,
}

impl Queue {
    /// Start one coordinator thread; active mutation workers are limited to two.
    ///
    /// # Errors
    /// Returns a thread/resource allocation error without starting any mutation.
    pub fn new() -> io::Result<Self> {
        let (sender, submissions) = mpsc::sync_channel(64);
        let (updates, receiver) = mpsc::sync_channel(64);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let handle = thread::Builder::new()
            .name("tfm-operations".into())
            .spawn(move || schedule(submissions, updates, stopping))?;
        Ok(Self {
            sender,
            receiver,
            stop,
            tokens: HashMap::new(),
            handle: Some(handle),
            next_id: 1,
        })
    }

    /// Submit a frozen plan without filesystem I/O or waiting on the UI thread.
    ///
    /// # Errors
    /// A full queue or disconnected coordinator rejects the request visibly.
    pub fn submit(&mut self, plan: Plan, options: Options) -> io::Result<u64> {
        self.submit_kind(plan, options, false)
    }

    /// Count and validate a recursive deletion scope without submitting mutations.
    ///
    /// # Errors
    /// Reports a full queue or unavailable coordinator.
    pub fn preview(&mut self, plan: Plan) -> io::Result<u64> {
        self.submit_kind(plan, Options::default(), true)
    }

    /// Reinspect a stopped unknown job under the same retained claims and identity.
    ///
    /// # Errors
    /// Refuses active jobs, resolved records, a full queue or an unavailable coordinator.
    pub fn reconcile(&mut self, id: u64, plan: Plan, record: Record) -> io::Result<()> {
        if self.tokens.contains_key(&id) || !record.reconcilable() || self.tokens.len() >= 64 {
            return Err(io::Error::other(
                "Only a stopped unknown job can be reconciled",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.sender
            .try_send(Submission {
                id,
                plan,
                options: Options::default(),
                cancel: cancel.clone(),
                preview: false,
                reconciliation: Some(record),
            })
            .map_err(|_| io::Error::other("Operation queue unavailable"))?;
        self.tokens.insert(id, cancel);
        Ok(())
    }

    fn submit_kind(&mut self, plan: Plan, options: Options, preview: bool) -> io::Result<u64> {
        if self.tokens.len() >= 64 {
            return Err(io::Error::other("Operation queue full (64 jobs)"));
        }
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Job identity exhausted"))?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.sender
            .try_send(Submission {
                id,
                plan,
                options,
                cancel: cancel.clone(),
                preview,
                reconciliation: None,
            })
            .map_err(|_| io::Error::other("Operation queue unavailable"))?;
        self.tokens.insert(id, cancel);
        Ok(id)
    }

    /// Request cancellation; completion is acknowledged separately through Finished.
    pub fn cancel(&self, id: u64) {
        if let Some(token) = self.tokens.get(&id) {
            token.store(true, Ordering::Relaxed);
        }
    }

    /// Stop submissions in every queued/active job; does not claim they have stopped.
    pub fn cancel_all(&self) {
        for token in self.tokens.values() {
            token.store(true, Ordering::Relaxed);
        }
    }

    /// Poll lifecycle events without blocking rendering; tests may use a bounded wait.
    pub fn next(&mut self, timeout: Duration) -> Option<Update> {
        let update = if timeout.is_zero() {
            self.receiver.try_recv().ok()
        } else {
            self.receiver.recv_timeout(timeout).ok()
        }?;
        if let Update::Finished { id, .. } | Update::Preview { id, .. } = &update {
            self.tokens.remove(id);
        }
        Some(update)
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        self.cancel_all();
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            while !handle.is_finished() {
                let _ = self.receiver.recv_timeout(Duration::from_millis(10));
            }
            let _ = handle.join();
        }
    }
}

fn schedule(
    submissions: mpsc::Receiver<Submission>,
    updates: mpsc::SyncSender<Update>,
    stop: Arc<AtomicBool>,
) {
    let (results, events) = mpsc::sync_channel(8);
    let mut pending = VecDeque::new();
    let mut active = BTreeMap::<u64, Running>::new();
    let mut coordinator = Coordinator::default();
    loop {
        while let Ok(message) = events.try_recv() {
            receive(message, &mut active, &mut coordinator, &updates);
        }
        for (id, worker) in &active {
            if let Ok(mut progress) = worker.progress.lock()
                && let Some(event) = progress.take()
            {
                let _ = updates.try_send(Update::Progress { id: *id, event });
            }
        }
        while let Ok(submission) = submissions.try_recv() {
            pending.push_back(submission);
        }
        pump(
            &mut pending,
            &mut active,
            &mut coordinator,
            &results,
            &updates,
            stop.load(Ordering::Relaxed),
        );
        if stop.load(Ordering::Relaxed) && active.is_empty() && pending.is_empty() {
            break;
        }
        match submissions.recv_timeout(Duration::from_millis(10)) {
            Ok(submission) => pending.push_back(submission),
            Err(mpsc::RecvTimeoutError::Disconnected) => stop.store(true, Ordering::Relaxed),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn receive(
    message: Message,
    active: &mut BTreeMap<u64, Running>,
    coordinator: &mut Coordinator,
    updates: &mpsc::SyncSender<Update>,
) {
    match message {
        Message::Update(update) => {
            acknowledge(&update, active, coordinator);
            let _ = updates.send(update);
        }
        Message::Claim {
            id,
            root,
            payload,
            accepted,
        } => {
            let _ = accepted.send(coordinator.own_stage(id, &root, &payload).unwrap_or(false));
        }
    }
}

fn acknowledge(
    update: &Update,
    active: &mut BTreeMap<u64, Running>,
    coordinator: &mut Coordinator,
) {
    let (id, unresolved) = match update {
        Update::Finished { id, record } => (*id, record.unresolved()),
        Update::Preview { id, .. } => (*id, false),
        _ => return,
    };
    if let Some(worker) = active.remove(&id) {
        let _ = worker.handle.join();
    }
    if unresolved {
        coordinator.quarantine(id);
    } else {
        coordinator.release(id);
    }
}

fn pump(
    pending: &mut VecDeque<Submission>,
    active: &mut BTreeMap<u64, Running>,
    coordinator: &mut Coordinator,
    events: &mpsc::SyncSender<Message>,
    updates: &mpsc::SyncSender<Update>,
    stopping: bool,
) {
    for _ in 0..pending.len() {
        let Some(job) = pending.pop_front() else {
            break;
        };
        if stopping {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if job.cancel.load(Ordering::Relaxed) {
            let _ = updates.send(Update::Finished {
                id: job.id,
                record: job.failed(
                    Outcome::Cancelled,
                    "Queued job cancelled; no mutations submitted",
                ),
            });
            continue;
        }
        let acquired = if job.reconciliation.is_some() {
            coordinator.resume(job.id)
        } else {
            coordinator.acquire(job.id, &job.plan)
        };
        match acquired {
            Ok(true) => start(job, active, coordinator, events, updates),
            Ok(false) => pending.push_back(job),
            Err(error) => {
                let _ = updates.send(Update::Finished {
                    id: job.id,
                    record: job.failed(Outcome::Failed, &error.to_string()),
                });
            }
        }
    }
}

fn start(
    job: Submission,
    active: &mut BTreeMap<u64, Running>,
    coordinator: &mut Coordinator,
    events: &mpsc::SyncSender<Message>,
    updates: &mpsc::SyncSender<Update>,
) {
    let progress = Arc::new(Mutex::new(None));
    let progress_writer = progress.clone();
    let events = events.clone();
    let id = job.id;
    let failed = job.failed(
        Outcome::OutcomeUnknown,
        "Mutation worker panicked; source/final/stage outcomes require reconciliation",
    );
    let not_started = job.failed(
        Outcome::Failed,
        "Worker could not start; no mutation submitted",
    );
    let spawn = thread::Builder::new()
        .name(format!("tfm-job-{id}"))
        .spawn(move || {
            let update = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(job, &events, &progress_writer)
            }))
            .unwrap_or(Update::Finished { id, record: failed });
            let _ = events.send(Message::Update(update));
        });
    match spawn {
        Ok(handle) => {
            active.insert(id, Running { handle, progress });
            let _ = updates.send(Update::Started { id });
        }
        Err(error) => {
            if not_started.unresolved() {
                coordinator.quarantine(id);
            } else {
                coordinator.release(id);
            }
            let mut record = not_started;
            for entry in &mut record.entries {
                entry.detail = format!("{}: {error}", entry.detail);
            }
            let _ = updates.send(Update::Finished { id, record });
        }
    }
}

fn run(
    mut job: Submission,
    events: &mpsc::SyncSender<Message>,
    progress: &Mutex<Option<Event>>,
) -> Update {
    if let Some(record) = job.reconciliation.take() {
        return Update::Finished {
            id: job.id,
            record: super::reconcile(&job.plan, record, &job.cancel),
        };
    }
    if job.preview {
        return prepare(job);
    }
    let record = super::execute(&job.plan, job.options, &job.cancel, |event| match event {
        Event::OwnedStage { root, payload } => claim(job.id, root, payload, events, &job.cancel),
        Event::Conflict { .. } => conflict(job.id, event, events, &job.cancel),
        Event::Progress { .. } => {
            *progress.lock().expect("owned progress") = Some(event);
            Decision::Skip
        }
    });
    Update::Finished { id: job.id, record }
}

fn conflict(
    id: u64,
    event: Event,
    events: &mpsc::SyncSender<Message>,
    cancel: &AtomicBool,
) -> Decision {
    let (reply, response) = mpsc::sync_channel(1);
    if events
        .send(Message::Update(Update::Conflict { id, event, reply }))
        .is_err()
    {
        return Decision::Cancel;
    }
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Decision::Cancel;
        }
        match response.recv_timeout(Duration::from_millis(25)) {
            Ok(decision) => return decision,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Decision::Cancel,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn claim(
    id: u64,
    root: Box<crate::fs::Entry>,
    payload: Box<crate::fs::Entry>,
    events: &mpsc::SyncSender<Message>,
    cancel: &AtomicBool,
) -> Decision {
    let (accepted, response) = mpsc::sync_channel(1);
    if events
        .send(Message::Claim {
            id,
            root,
            payload,
            accepted,
        })
        .is_err()
    {
        return Decision::Cancel;
    }
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Decision::Cancel;
        }
        match response.recv_timeout(Duration::from_millis(25)) {
            Ok(true) => return Decision::Skip,
            Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => return Decision::Cancel,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

pub(super) fn unsuccessful(plan: &Plan, outcome: Outcome, detail: &str) -> Record {
    let mut record = Record {
        entries: plan
            .operands
            .iter()
            .map(|source| EntryResult {
                recovery: None,
                trash: None,
                source: source.path.clone(),
                destination: plan.destination.clone(),
                kind: source.kind,
                outcome,
                bytes: 0,
                checksum_verified: false,
                created_directory: false,
                detail: detail.into(),
            })
            .collect(),
    };
    if record.entries.is_empty() {
        record.entries.push(EntryResult {
            recovery: None,
            trash: None,
            source: plan.destination.clone().unwrap_or_default(),
            destination: plan.destination.clone(),
            kind: crate::fs::EntryKind::Directory,
            outcome,
            bytes: 0,
            checksum_verified: false,
            created_directory: false,
            detail: detail.into(),
        });
    }
    record
}

fn prepare(mut job: Submission) -> Update {
    if let Err(error) = job.plan.refresh_retry() {
        return Update::Finished {
            id: job.id,
            record: unsuccessful(&job.plan, Outcome::Changed, &error.to_string()),
        };
    }
    match super::Preview::capture(&job.plan, &job.cancel) {
        Ok(preview) => {
            job.plan.preview = Some(preview);
            Update::Preview {
                id: job.id,
                plan: Box::new(job.plan),
            }
        }
        Err(error) => Update::Finished {
            id: job.id,
            record: unsuccessful(&job.plan, error.outcome, &error.detail),
        },
    }
}

#[cfg(test)]
pub(super) mod tests;
