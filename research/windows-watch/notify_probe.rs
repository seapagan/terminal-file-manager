use notify::{EventKind, RecursiveMode, Watcher};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let root = PathBuf::from(&args[0]);
    let current = PathBuf::from(&args[1]);
    let recursive = args[2] == "root";
    let relevant = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let (count, all, path) = (relevant.clone(), total.clone(), current.clone());
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        all.fetch_add(1, Ordering::Relaxed);
        if invalidates(result, &path) {
            count.fetch_add(1, Ordering::Relaxed);
        }
    })?;
    register(&mut watcher, &root, &current, recursive)?;
    println!("READY");
    io::stdout().flush()?;
    let (stop, receive) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = io::stdin().lock().lines().next();
        let _ = stop.send(());
    });
    let forwarded = forward(&receive, &relevant, &total)?;
    drop(watcher);
    println!(
        "STATS {} {}",
        total.load(Ordering::Relaxed),
        forwarded + relevant.load(Ordering::Relaxed)
    );
    io::stdout().flush()?;
    Ok(())
}

fn invalidates(result: notify::Result<notify::Event>, path: &std::path::Path) -> bool {
    let Ok(event) = result else { return true };
    if matches!(
        event.kind,
        EventKind::Access(_)
            | EventKind::Modify(notify::event::ModifyKind::Metadata(
                notify::event::MetadataKind::AccessTime
            ))
    ) {
        return false;
    }
    event.need_rescan()
        || event.paths.is_empty()
        || event
            .paths
            .iter()
            .any(|p| p == path || p.parent() == Some(path) || path.starts_with(p))
}

fn forward(
    receive: &mpsc::Receiver<()>,
    relevant: &AtomicU64,
    total: &AtomicU64,
) -> io::Result<u64> {
    let mut forwarded = 0;
    while let Err(mpsc::RecvTimeoutError::Timeout) = receive.recv_timeout(Duration::from_millis(10))
    {
        let hints = relevant.swap(0, Ordering::Relaxed);
        if hints > 0 {
            forwarded += hints;
            println!("HINT {hints} {}", total.load(Ordering::Relaxed));
            io::stdout().flush()?;
        }
    }
    Ok(forwarded)
}

fn register(
    watcher: &mut impl Watcher,
    root: &std::path::Path,
    current: &std::path::Path,
    recursive: bool,
) -> notify::Result<()> {
    if recursive {
        watcher.watch(root, RecursiveMode::Recursive)?;
    } else {
        watcher.watch(current, RecursiveMode::NonRecursive)?;
        watcher.watch(current.parent().unwrap(), RecursiveMode::NonRecursive)?;
    }
    Ok(())
}
