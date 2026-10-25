use crate::watch::schedule::Backoff;
use std::time::Duration;

/// A successful syscall is not recovery until the subscription stays stable.
#[derive(Default)]
pub(super) struct Recovery {
    pub(super) retry: Duration,
    backoff: Backoff,
    since: Option<Duration>,
    degraded: bool,
}

impl Recovery {
    pub(super) fn failed(&mut self, now: Duration) {
        self.degraded = true;
        self.since = None;
        self.retry = now + self.backoff.next();
    }

    pub(super) fn registered(&mut self, now: Duration) -> bool {
        self.since = Some(now);
        !self.degraded
    }

    pub(super) fn recovered(&mut self, now: Duration) -> bool {
        if self
            .since
            .is_none_or(|since| now < since + Duration::from_secs(30))
        {
            return false;
        }
        self.backoff = Backoff::default();
        std::mem::take(&mut self.degraded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    #[test]
    fn immediate_backend_errors_and_rescans_do_not_reset_backoff_or_report_recovery() {
        let mut recovery = Recovery::default();
        assert!(recovery.registered(s(0)));
        let mut now = s(0);
        for delay in [1, 2, 4, 8, 16, 30, 30] {
            recovery.failed(now);
            assert_eq!(recovery.retry, now + s(delay));
            now = recovery.retry;
            assert!(!recovery.registered(now));
            assert!(!recovery.recovered(now));
        }
        assert!(!recovery.recovered(now + s(29)));
        assert!(recovery.recovered(now + s(30)));
        assert!(!recovery.recovered(now + s(31)));
        recovery.failed(now + s(31));
        assert_eq!(recovery.retry, now + s(32));
    }

    #[test]
    fn registration_failures_and_subthreshold_stability_share_the_capped_backoff() {
        let mut recovery = Recovery::default();
        recovery.failed(s(0));
        recovery.failed(s(1));
        assert_eq!(recovery.retry, s(3));
        assert!(!recovery.registered(s(3)));
        assert!(!recovery.recovered(s(32)));
        recovery.failed(s(32));
        assert_eq!(recovery.retry, s(36));
    }
}
