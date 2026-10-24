//! Bounded read-only resource sampling for owned Windows test processes.
#![cfg(windows)]
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Counters {
    pub cpu_ticks: u64,
    pub rss: u64,
    pub handles: u64,
    sampled: u64,
    frequency: u64,
}

impl Counters {
    pub fn elapsed_to(&self, after: &Self) -> Duration {
        assert!(self.frequency > 0 && self.frequency == after.frequency);
        Duration::from_secs_f64((after.sampled - self.sampled) as f64 / self.frequency as f64)
    }
}

pub fn counters(process: u32) -> Counters {
    let script = format!(
        "$p=[System.Diagnostics.Process]::GetProcessById({}); try {{ $sample=[System.Diagnostics.Stopwatch]::GetTimestamp(); [Console]::WriteLine($p.TotalProcessorTime.Ticks); [Console]::WriteLine($p.WorkingSet64); [Console]::WriteLine($p.HandleCount); [Console]::WriteLine($sample); [Console]::WriteLine([System.Diagnostics.Stopwatch]::Frequency) }} finally {{ $p.Dispose() }}",
        process
    );
    let mut child = Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned-process counter query timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "counter query failed: {:?}",
        result
    );
    let values: Vec<u64> = std::str::from_utf8(&result.stdout)
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();
    assert_eq!(values.len(), 5);
    Counters {
        cpu_ticks: values[0],
        rss: values[1],
        handles: values[2],
        sampled: values[3],
        frequency: values[4],
    }
}
