//! Session events own history; transient toast queues contain only event identities.
use std::{
    collections::VecDeque,
    time::{Duration, SystemTime},
};

/// Severity order also determines toast priority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
    pub fn role(self) -> crate::color::Role {
        match self {
            Self::Info => crate::color::Role::Value,
            Self::Success => crate::color::Role::Status,
            Self::Warning => crate::color::Role::Warning,
            Self::Error => crate::color::Role::Error,
        }
    }
}

/// Placement is relative to available content, excluding persistent controls.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

/// One retained event, optionally linking the authoritative operation evidence.
pub struct Notice {
    pub text: String,
    pub severity: Severity,
    pub created: SystemTime,
    pub occurrences: usize,
    pub job: Option<u64>,
    expires: Duration,
    updated: Duration,
}

/// History is session-long; only the immediate presentation queue is bounded.
pub struct Notifications {
    pub history: Vec<Notice>,
    pending: VecDeque<usize>,
    clock: Duration,
    latest: Option<usize>,
    pub placement: Placement,
    pub expiry_seconds: u16,
    pub open: bool,
    pub cursor: usize,
    pub scroll: usize,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            pending: VecDeque::new(),
            clock: Duration::ZERO,
            latest: None,
            placement: Placement::default(),
            expiry_seconds: 5,
            open: false,
            cursor: 0,
            scroll: 0,
        }
    }
}

impl Notifications {
    /// Publish an application event without manufacturing a filesystem job.
    pub fn notify(&mut self, text: String, now: SystemTime, severity: Severity) {
        self.record(text, now, severity, None);
    }

    /// A job's final notification links to its existing record, including reconciliation.
    pub fn record(&mut self, text: String, now: SystemTime, severity: Severity, job: Option<u64>) {
        let duplicate = self.matching_event(&text, severity, job);
        let expires = self.clock.saturating_add(self.lifetime());
        let index = if let Some(index) = duplicate {
            let event = &mut self.history[index];
            event.occurrences = event.occurrences.saturating_add(1);
            event.text = text;
            event.severity = severity;
            event.created = now;
            event.expires = expires;
            event.updated = self.clock;
            index
        } else {
            self.history.push(Notice {
                text,
                severity,
                created: now,
                occurrences: 1,
                job,
                expires,
                updated: self.clock,
            });
            self.history.len() - 1
        };
        self.latest = Some(index);
        self.pending
            .retain(|id| *id != index && (job.is_none() || self.history[*id].job != job));
        self.pending.push_back(index);
        if self.pending.len() > 16 {
            let lowest = self
                .pending
                .iter()
                .copied()
                .min_by_key(|id| self.history[*id].severity)
                .unwrap();
            self.pending.retain(|id| *id != lowest);
        }
    }

    fn matching_event(&self, text: &str, severity: Severity, job: Option<u64>) -> Option<usize> {
        self.history.iter().rposition(|event| {
            if let Some(id) = job {
                event.job == Some(id) && event.text == text && event.severity == severity
            } else {
                event.job.is_none()
                    && event.text == text
                    && event.severity == severity
                    && self.clock.saturating_sub(event.updated) <= Duration::from_secs(2)
            }
        })
    }

    fn lifetime(&self) -> Duration {
        Duration::from_secs(u64::from(self.expiry_seconds.clamp(1, 60)))
    }

    /// Advance an injected monotonic clock. Expiry changes presentation only.
    pub fn advance(&mut self, elapsed: Duration) -> bool {
        self.clock = self.clock.max(elapsed);
        let before = self.pending.len();
        self.pending
            .retain(|id| self.history[*id].expires > self.clock);
        before != self.pending.len()
    }

    /// Last published event, including a refreshed deduplication group.
    pub fn latest_index(&self) -> Option<usize> {
        self.latest
    }

    /// Hide transient presentations without removing retained events.
    pub fn dismiss_toasts(&mut self) {
        self.pending.clear();
    }

    /// Highest priority first; equally important recent events appear first.
    pub fn visible(&self) -> Vec<usize> {
        let mut ids: Vec<_> = self
            .pending
            .iter()
            .rev()
            .copied()
            .filter(|id| self.history[*id].expires > self.clock)
            .collect();
        ids.sort_by_key(|id| std::cmp::Reverse(self.history[*id].severity));
        ids.truncate(3);
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_unique_burst_bounds_pending_presentations_without_discarding_history() {
        let mut notices = Notifications::default();
        for index in 0..100 {
            notices.notify(index.to_string(), SystemTime::UNIX_EPOCH, Severity::Info);
        }
        assert_eq!(notices.pending.len(), 16);
        assert_eq!(notices.history.len(), 100);
        assert_eq!(notices.visible(), [99, 98, 97]);
    }
}
