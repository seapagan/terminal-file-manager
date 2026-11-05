//! Session events own history; transient toast queues contain only event identities.
mod presentation;
pub use presentation::Presentation;
use std::{
    cell::RefCell,
    collections::VecDeque,
    time::{Duration, SystemTime},
};

/// Severity controls styling; presentation order is chronological.
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
            Self::Info => crate::color::Role::Link,
            Self::Success => crate::color::Role::Status,
            Self::Warning => crate::color::Role::Warning,
            Self::Error => crate::color::Role::Error,
        }
    }
}

/// Placement is relative to the terminal, above underlying interfaces.
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
    pub presentation: Presentation,
    pub severity: Severity,
    pub created: SystemTime,
    pub occurrences: usize,
    pub job: Option<u64>,
    expires: Duration,
    updated: Duration,
    key: String,
}

impl Notice {
    fn same_toast(&self, other: &Self) -> bool {
        self.severity == other.severity
            && self.key == other.key
            && self.updated.abs_diff(other.updated) <= Duration::from_secs(2)
    }
}

/// History is session-long; terminal geometry bounds transient presentation.
pub struct Notifications {
    pub history: Vec<Notice>,
    pending: RefCell<VecDeque<usize>>,
    clock: Duration,
    latest: Option<usize>,
    pub placement: Placement,
    pub expiry_seconds: u16,
    pub open: bool,
    pub cursor: usize,
    pub scroll: usize,
    pub details: bool,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            pending: RefCell::new(VecDeque::new()),
            clock: Duration::ZERO,
            latest: None,
            placement: Placement::default(),
            expiry_seconds: 5,
            open: false,
            cursor: 0,
            scroll: 0,
            details: false,
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
        let presentation = Presentation::application(&text, severity);
        let key = text.clone();
        self.publish(text, now, severity, job, presentation, key);
    }

    fn publish(
        &mut self,
        text: String,
        now: SystemTime,
        severity: Severity,
        job: Option<u64>,
        presentation: Presentation,
        key: String,
    ) {
        let follow = self.open && self.cursor == self.history.len().saturating_sub(1);
        let duplicate = self.matching_event(&text, severity, job);
        let expires = self.clock.saturating_add(self.lifetime());
        let index = if let Some(index) = duplicate {
            let event = &mut self.history[index];
            event.occurrences = event.occurrences.saturating_add(1);
            event.text = text;
            event.presentation = presentation;
            event.key = key;
            event.severity = severity;
            event.created = now;
            event.expires = expires;
            event.updated = self.clock;
            index
        } else {
            self.history.push(Notice {
                presentation,
                key,
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
        if follow && duplicate.is_none() {
            self.cursor = index;
            self.scroll = 0;
        }
        self.present(index, job);
    }

    fn present(&mut self, index: usize, job: Option<u64>) {
        if self.open {
            self.dismiss_toasts();
            return;
        }
        let history = &self.history;
        let pending = self.pending.get_mut();
        pending.retain(|id| {
            *id != index
                && (job.is_none() || history[*id].job != job)
                && !history[*id].same_toast(&history[index])
        });
        pending.push_back(index);
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
        let pending = self.pending.get_mut();
        let before = pending.len();
        pending.retain(|id| self.history[*id].expires > self.clock);
        before != pending.len()
    }

    /// Last published event, including a refreshed deduplication group.
    pub fn latest_index(&self) -> Option<usize> {
        self.latest
    }

    /// Hide transient presentations without removing retained events.
    pub fn dismiss_toasts(&mut self) {
        self.pending.get_mut().clear();
    }

    /// Dismiss one visual group; independently retained events and Jobs survive.
    pub fn dismiss_toast(&mut self, id: usize) {
        self.pending.get_mut().retain(|event| *event != id);
    }

    /// Rendering retires displaced presentations so resizing cannot revive them.
    pub(crate) fn retain_toasts(&self, visible: &[usize]) {
        self.pending.borrow_mut().retain(|id| visible.contains(id));
    }

    /// Newest presentations first, nearest the configured corner.
    pub fn visible(&self) -> Vec<usize> {
        self.pending
            .borrow()
            .iter()
            .rev()
            .copied()
            .filter(|id| self.history[*id].expires > self.clock)
            .collect()
    }

    /// Presentation counts never merge the independently retained Job links.
    pub fn toast_occurrences(&self, id: usize) -> usize {
        self.history
            .iter()
            .filter(|event| self.history[id].same_toast(event))
            .map(|event| event.occurrences)
            .fold(0, usize::saturating_add)
    }

    pub(crate) fn record_job(
        &mut self,
        text: String,
        now: SystemTime,
        severity: Severity,
        job: &crate::operations::Job,
    ) {
        self.publish(
            text,
            now,
            severity,
            Some(job.id),
            Presentation::job(job),
            presentation::job_key(job),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_unique_burst_retains_presentations_until_geometry_budgets_them() {
        let mut notices = Notifications::default();
        for index in 0..100 {
            notices.notify(index.to_string(), SystemTime::UNIX_EPOCH, Severity::Info);
        }
        assert_eq!(notices.pending.borrow().len(), 100);
        assert_eq!(notices.history.len(), 100);
        assert_eq!(notices.visible(), (0..100).rev().collect::<Vec<_>>());
    }
}
