use std::time::Duration;

pub(super) struct Backoff(Duration);

impl Default for Backoff {
    fn default() -> Self {
        Self(Duration::from_secs(1))
    }
}

impl Backoff {
    pub(super) fn next(&mut self) -> Duration {
        let delay = self.0;
        self.0 = (delay * 2).min(Duration::from_secs(30));
        delay
    }
}

#[derive(Default)]
pub(super) struct Schedule {
    first: Option<Duration>,
    last: Duration,
    started: Option<Duration>,
}

impl Schedule {
    pub(super) fn dirty(&mut self, now: Duration) {
        self.first.get_or_insert(now);
        self.last = now;
    }
    pub(super) fn due(&self, now: Duration) -> bool {
        self.first.is_some_and(|first| {
            (now >= self.last + Duration::from_millis(200) || now >= first + Duration::from_secs(1))
                && self
                    .started
                    .is_none_or(|start| now >= start + Duration::from_secs(1))
        })
    }
    pub(super) fn started(&mut self, now: Duration) {
        self.first = None;
        self.started = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn isolated_change_waits_for_quiet_window() {
        let mut schedule = Schedule::default();
        schedule.dirty(ms(10));
        assert!(!schedule.due(ms(209)));
        assert!(schedule.due(ms(210)));
        schedule.started(ms(210));
        assert!(!schedule.due(ms(10000)));
    }

    #[test]
    fn continuous_events_start_within_one_second_then_throttle_and_catch_up() {
        let mut schedule = Schedule::default();
        for time in (0..=1000).step_by(10) {
            schedule.dirty(ms(time));
            assert_eq!(schedule.due(ms(time)), time == 1000);
        }
        schedule.started(ms(1000));
        for time in (1010..2000).step_by(10) {
            schedule.dirty(ms(time));
            assert!(!schedule.due(ms(time)));
        }
        assert!(!schedule.due(ms(1999)));
        assert!(schedule.due(ms(2190)));
        schedule.started(ms(2190));
        assert!(!schedule.due(ms(60000)));
    }

    #[test]
    fn event_count_does_not_change_deadlines_or_lose_late_dirty_state() {
        for count in [1, 100, 5000] {
            let mut schedule = Schedule::default();
            for _ in 0..count {
                schedule.dirty(ms(0));
            }
            assert!(!schedule.due(ms(199)));
            assert!(schedule.due(ms(200)));
            schedule.started(ms(200));
            schedule.dirty(ms(201));
            assert!(!schedule.due(ms(1199)));
            assert!(schedule.due(ms(1200)));
        }
    }
}
