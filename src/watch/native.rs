use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) const DIRTY: u8 = 1;
pub(super) const READY: u8 = 2;
pub(super) const ERROR: u8 = 4;

#[derive(Default)]
pub(super) struct Signals {
    pub(super) bits: AtomicU8,
    failed: AtomicBool,
}

impl Signals {
    pub(super) fn publish(&self, bits: u8) {
        if bits & (READY | ERROR) == 0 {
            self.bits.fetch_or(bits, Ordering::Release);
        } else {
            let _ = self
                .bits
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |previous| {
                    Some((previous & !(READY | ERROR)) | bits)
                });
        }
    }
}

pub(super) struct Binding {
    pub(super) path: PathBuf,
    pub(super) signals: Arc<Signals>,
}

/// Registration and teardown never run on the input/render thread.
pub(super) struct Worker {
    pending: Arc<Mutex<Option<Binding>>>,
    wake: mpsc::SyncSender<()>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub(super) fn new() -> std::io::Result<Self> {
        let pending = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (wake, receive) = mpsc::sync_channel(1);
        let shared = pending.clone();
        let stopping = stop.clone();
        let handle = thread::Builder::new()
            .name("tfm-watch".into())
            .spawn(move || run(shared, stopping, receive))?;
        Ok(Self {
            pending,
            wake,
            stop,
            handle: Some(handle),
        })
    }

    pub(super) fn bind(&self, binding: Binding) {
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(binding);
        let _ = self.wake.try_send(());
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(handle) = self.handle.take().filter(|h| h.is_finished()) {
            let _ = handle.join();
        }
        // A filesystem syscall may stall; its single owner exits when it returns.
    }
}

fn run(pending: Arc<Mutex<Option<Binding>>>, stop: Arc<AtomicBool>, receive: mpsc::Receiver<()>) {
    let mut binding: Option<Binding> = None;
    let mut watcher = None;
    let mut retry = Instant::now();
    let mut backoff = Duration::from_secs(1);
    while !stop.load(Ordering::Acquire) {
        let next = pending.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(next) = next {
            watcher = None;
            binding = Some(next);
            retry = Instant::now();
            backoff = Duration::from_secs(1);
        }
        if let Some(current) = &binding {
            if current.signals.failed.swap(false, Ordering::AcqRel) {
                current.signals.publish(ERROR | DIRTY);
                watcher = None;
                retry = Instant::now() + backoff;
            }
            if watcher.is_none() && Instant::now() >= retry {
                watcher = register(current).ok();
                if watcher.is_some() {
                    current.signals.publish(READY | DIRTY);
                    backoff = Duration::from_secs(1);
                } else {
                    current.signals.publish(ERROR | DIRTY);
                    retry = Instant::now() + backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }
        let _ = receive.recv_timeout(Duration::from_millis(100));
    }
}

fn register(binding: &Binding) -> notify::Result<RecommendedWatcher> {
    let raw = binding.path.clone();
    let physical = std::fs::canonicalize(&raw).map_err(notify::Error::io)?;
    let signals = binding.signals.clone();
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event) if event.need_rescan() => {
                signals.failed.store(true, Ordering::Release);
                signals.publish(ERROR | DIRTY);
            }
            Ok(event) if relevant(&event, &raw, &physical) => {
                signals.publish(DIRTY);
            }
            Err(_) => {
                signals.failed.store(true, Ordering::Release);
                signals.publish(ERROR | DIRTY);
            }
            _ => {}
        })?;
    watcher.watch(&binding.path, RecursiveMode::NonRecursive)?;
    if let Some(parent) = binding.path.parent().filter(|p| !p.as_os_str().is_empty()) {
        watcher.watch(parent, RecursiveMode::NonRecursive)?;
    }
    Ok(watcher)
}

fn relevant(event: &Event, raw: &Path, physical: &Path) -> bool {
    use notify::event::{MetadataKind, ModifyKind};
    if event.need_rescan() {
        return true;
    }
    if matches!(
        event.kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))
    ) {
        return false;
    }
    event.paths.is_empty()
        || event.paths.iter().any(|path| {
            [raw, physical].iter().any(|directory| {
                path == directory
                    || path.parent() == Some(directory)
                    || Some(path.as_path()) == directory.parent()
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, AccessMode, Flag, MetadataKind, ModifyKind};

    fn wait_for(signals: &Signals, mask: u8) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while signals.bits.load(Ordering::Acquire) & mask == 0 {
            assert!(Instant::now() < deadline, "native watch state timed out");
            thread::yield_now();
        }
    }

    #[test]
    fn native_failed_registration_retries_and_shutdown_releases_its_owner() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("temporarily-missing");
        let signals = Arc::<Signals>::default();
        let mut worker = Worker::new().unwrap();
        worker.bind(Binding {
            path: path.clone(),
            signals: signals.clone(),
        });
        wait_for(&signals, ERROR);
        std::fs::create_dir(&path).unwrap();
        wait_for(&signals, READY);
        let owner = worker.handle.take().unwrap();
        let stopped = Instant::now();
        drop(worker);
        let deadline = stopped + Duration::from_secs(5);
        while !owner.is_finished() {
            assert!(
                Instant::now() < deadline,
                "watch owner leaked after shutdown"
            );
            thread::yield_now();
        }
        owner.join().unwrap();
        eprintln!(
            "Native failed registration recovered; owner shutdown {:?}",
            stopped.elapsed()
        );
    }

    #[test]
    fn native_rebind_keeps_only_latest_pending_subscription_and_handles_roots() {
        let root = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let worker = Worker::new().unwrap();
        for index in 0..100 {
            worker.bind(Binding {
                path: root.path().join(format!("old-{index}")),
                signals: Arc::default(),
            });
        }
        let signals = Arc::<Signals>::default();
        worker.bind(Binding {
            path: root.path().into(),
            signals: signals.clone(),
        });
        wait_for(&signals, READY);
        eprintln!(
            "Native latest-only registration/rebind: {:?}",
            started.elapsed()
        );
        let filesystem_root = root.path().ancestors().last().unwrap();
        assert!(filesystem_root.parent().is_none());
        let event = Event::new(EventKind::Any).add_path(filesystem_root.join("child"));
        assert!(relevant(&event, filesystem_root, filesystem_root));
    }

    #[test]
    fn reads_and_unrelated_parent_children_do_not_invalidate() {
        let raw = Path::new("/alias/current");
        let physical = Path::new("/physical/current");
        for kind in [
            EventKind::Access(AccessKind::Read),
            EventKind::Access(AccessKind::Open(AccessMode::Read)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)),
        ] {
            let event = Event::new(kind).add_path(raw.join("file"));
            assert!(!relevant(&event, raw, physical));
        }
        assert!(!relevant(
            &Event::new(EventKind::Any).add_path("/alias/sibling/file".into()),
            raw,
            physical
        ));
        assert!(!relevant(
            &Event::new(EventKind::Any).add_path(raw.join("child/grandchild")),
            raw,
            physical
        ));
    }

    #[test]
    fn mutations_aliases_empty_paths_and_overflow_conservatively_invalidate() {
        let raw = Path::new("/alias/current");
        let physical = Path::new("/physical/current");
        for path in [raw.to_path_buf(), raw.join("file"), physical.join("file")] {
            for kind in [
                EventKind::Create(notify::event::CreateKind::File),
                EventKind::Remove(notify::event::RemoveKind::File),
                EventKind::Modify(ModifyKind::Any),
            ] {
                assert!(relevant(
                    &Event::new(kind).add_path(path.clone()),
                    raw,
                    physical
                ));
            }
        }
        assert!(relevant(&Event::new(EventKind::Any), raw, physical));
        assert!(relevant(
            &Event::new(EventKind::Other).set_flag(Flag::Rescan),
            raw,
            physical
        ));
    }
}
