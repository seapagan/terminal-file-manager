//! Fixed workers and bounded channels; the UI only uses try_send/try_recv.
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

const WORKERS: usize = 2;
type Inbox<K> = Arc<Mutex<mpsc::Receiver<Task<K>>>>;

pub(super) struct Task<K> {
    pub key: K,
    epoch: u64,
    cancelled: Arc<AtomicBool>,
}

pub(super) struct Reply<K, V> {
    pub key: K,
    pub epoch: u64,
    pub value: Option<Result<V, String>>,
}

pub(super) struct Pool<K, V> {
    send: mpsc::SyncSender<Task<K>>,
    receive: mpsc::Receiver<Reply<K, V>>,
    pub active: Vec<Task<K>>,
}

impl<K: Clone + Eq + Send + 'static, V: Send + 'static> Pool<K, V> {
    pub fn new(name: &str, execute: impl Fn(&K) -> V + Send + Sync + 'static) -> io::Result<Self> {
        let (send, receive) = mpsc::sync_channel(WORKERS);
        let (replies, results) = mpsc::sync_channel(WORKERS);
        let inbox = Arc::new(Mutex::new(receive));
        let execute = Arc::new(execute);
        for index in 0..WORKERS {
            let (inbox, replies, execute) = (inbox.clone(), replies.clone(), execute.clone());
            // Dropping the join handle is deliberate: an uninterruptible native
            // read must never hold terminal shutdown hostage. Channel closure
            // stops idle workers and retires busy workers when their call ends.
            std::thread::Builder::new()
                .name(format!("tfm-owner-{name}-{index}"))
                .spawn(move || serve(inbox, replies, execute))?;
        }
        Ok(Self {
            send,
            receive: results,
            active: Vec::with_capacity(WORKERS),
        })
    }

    pub fn start(&mut self, key: K, epoch: u64) -> Result<bool, String> {
        if self.active.len() == WORKERS
            || self
                .active
                .iter()
                .any(|job| job.key == key && job.epoch == epoch)
        {
            return Ok(false);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let task = Task {
            key: key.clone(),
            epoch,
            cancelled: cancelled.clone(),
        };
        self.send
            .try_send(task)
            .map_err(|error| format!("Ownership worker unavailable: {error}"))?;
        self.active.push(Task {
            key,
            epoch,
            cancelled,
        });
        Ok(true)
    }

    pub fn cancel_except(&self, wanted: &[K], epoch: u64) {
        for job in &self.active {
            if job.epoch != epoch || !wanted.contains(&job.key) {
                job.cancelled.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn take(&mut self) -> Vec<Reply<K, V>> {
        let mut ready = Vec::with_capacity(WORKERS);
        while let Ok(mut reply) = self.receive.try_recv() {
            if let Some(index) = self
                .active
                .iter()
                .position(|job| job.key == reply.key && job.epoch == reply.epoch)
            {
                let job = self.active.swap_remove(index);
                if job.cancelled.load(Ordering::Relaxed) {
                    reply.value = None;
                }
            }
            ready.push(reply);
        }
        ready
    }
}

impl<K, V> Drop for Pool<K, V> {
    fn drop(&mut self) {
        for job in &self.active {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }
}

fn serve<K, V, F>(inbox: Inbox<K>, replies: mpsc::SyncSender<Reply<K, V>>, execute: Arc<F>)
where
    F: Fn(&K) -> V,
{
    loop {
        // Only workers acquire this mutex. Release it before any native call.
        let next = inbox.lock().expect("owner inbox").recv();
        let Ok(job) = next else {
            break;
        };
        let mut value = None;
        if !job.cancelled.load(Ordering::Relaxed) {
            value = Some(
                catch_unwind(AssertUnwindSafe(|| execute(&job.key)))
                    .map_err(|_| "Ownership worker panicked".into()),
            );
        }
        if job.cancelled.load(Ordering::Relaxed) {
            value = None;
        }
        if replies
            .send(Reply {
                key: job.key,
                epoch: job.epoch,
                value,
            })
            .is_err()
        {
            break;
        }
    }
}
