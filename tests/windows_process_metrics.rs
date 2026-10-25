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
        "$ErrorActionPreference='Stop'; $p=[System.Diagnostics.Process]::GetProcessById({}); try {{ $sample=[System.Diagnostics.Stopwatch]::GetTimestamp(); [Console]::WriteLine($p.TotalProcessorTime.Ticks); [Console]::WriteLine($p.WorkingSet64); [Console]::WriteLine($p.HandleCount); [Console]::WriteLine($sample); [Console]::WriteLine([System.Diagnostics.Stopwatch]::Frequency) }} finally {{ $p.Dispose() }}",
        process
    );
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start Windows PowerShell process counter sampler");
    // Allow slower PowerShell completion on shared runners while bounding stalled queries.
    const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() >= QUERY_TIMEOUT {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "owned-process counter query timed out after {:?}; stdout: {}; stderr: {}",
                started.elapsed(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
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
        .expect("counter sampler returned non-UTF8 output")
        .split_whitespace()
        .map(|value| {
            value
                .parse()
                .expect("counter sampler returned an invalid number")
        })
        .collect();
    assert_eq!(values.len(), 5, "counter sampler must return five counters");
    Counters {
        cpu_ticks: values[0],
        rss: values[1],
        handles: values[2],
        sampled: values[3],
        frequency: values[4],
    }
}
