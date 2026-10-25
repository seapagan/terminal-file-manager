//! ConPTY manual-refresh and idle-cost acceptance using owned Windows fixtures.
#![cfg(windows)]
#[path = "terminal_driver.rs"]
mod terminal_driver;
#[path = "windows_process_metrics.rs"]
pub mod windows_process_metrics;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use windows_process_metrics::counters;

fn terminal_command(root: &Path) -> portable_pty::CommandBuilder {
    let mut command = portable_pty::CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.arg(root);
    command.cwd(root);
    for variable in ["HOME", "USERPROFILE"] {
        command.env(variable, root);
    }
    for (variable, directory) in [
        ("XDG_CONFIG_HOME", "config"),
        ("APPDATA", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        command.env(variable, root.join(directory));
    }
    command.env("GIT_CONFIG_GLOBAL", root.join("gitconfig"));
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    command.env("NO_COLOR", "1");
    command.env("TERM", "xterm-256color");
    command.env_remove("VTE_VERSION");
    command
}

fn terminal_observe(
    session: &mut terminal_driver::Session,
    transcript: &mut Vec<u8>,
    duration: Duration,
) {
    let mut script = terminal_driver::Script::default();
    let started = Instant::now();
    while started.elapsed() < duration {
        session
            .read_step(
                Duration::from_millis(20),
                &mut script,
                &[],
                transcript,
                false,
            )
            .unwrap();
    }
}

fn terminal_idle(entries: usize, session: &mut terminal_driver::Session, transcript: &mut Vec<u8>) {
    let process = session
        .child
        .process_id()
        .expect("owned native TUI process ID");
    terminal_observe(session, transcript, Duration::from_secs(4));
    let before = counters(process);
    let idle = if cfg!(debug_assertions) {
        Duration::from_millis(4200)
    } else {
        Duration::from_secs(30)
    };
    terminal_observe(session, transcript, idle);
    let after = counters(process);
    let wall = before.elapsed_to(&after);
    let cpu_ms = (after.cpu_ticks - before.cpu_ticks) as f64 / 10000.0;
    eprintln!(
        "Windows manual TUI {entries}: debug_assertions={}, sampled_wall_s={}, cpu_ms={cpu_ms}, one_core_pct={}, rss_before={}, rss_after={}, handles_before={}, handles_after={}",
        cfg!(debug_assertions),
        wall.as_secs_f64(),
        cpu_ms / wall.as_secs_f64() / 10.0,
        before.rss,
        after.rss,
        before.handles,
        after.handles
    );
}

pub fn sample(entries: usize, root: &Path) {
    use terminal_driver::{Script, Session};
    for index in 0..3 {
        std::fs::remove_file(root.join(format!("isolated-{index}"))).unwrap();
    }
    let mut session = Session::open(terminal_command(root)).unwrap();
    let mut transcript = Vec::new();
    let mut script = Script::default();
    let deadline = Instant::now() + Duration::from_secs(20);
    while script.step == 0 {
        assert!(Instant::now() < deadline, "native TUI listing timed out");
        session
            .read_step(
                Duration::from_millis(20),
                &mut script,
                &[("item00000", "")],
                &mut transcript,
                false,
            )
            .unwrap();
    }
    let marker = format!("!manual-visible-{entries}");
    std::fs::write(root.join(&marker), "external").unwrap();
    terminal_idle(entries, &mut session, &mut transcript);
    assert!(
        !String::from_utf8_lossy(&transcript).contains(&marker),
        "external create appeared before r"
    );
    let changed = Instant::now();
    use std::io::Write;
    let input = session.writer.as_mut().unwrap();
    input.write_all(b"r").unwrap();
    input.flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    session
        .drive(&[(&marker, "q")], deadline, &mut transcript, false)
        .unwrap();
    eprintln!(
        "Windows manual TUI {entries}: r-to-render-and-exit {:?}",
        changed.elapsed()
    );
    drop(session);
    terminal_driver::drive(terminal_command(root), &[(&marker, "q")], false);
}
