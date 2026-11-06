//! Session events own history; transient toast queues contain only event identities.
mod presentation;
pub use presentation::Presentation;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, SystemTime},
};

/// Severity controls styling; presentation order is chronological.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    group: usize,
}

struct ToastGroup {
    updated: Duration,
    occurrences: usize,
}

#[derive(Default)]
struct Indexes {
    events: HashMap<(Option<u64>, Severity, String), usize>,
    latest_groups: HashMap<(Severity, String), usize>,
    groups: Vec<ToastGroup>,
}

/// History is session-long; terminal geometry bounds transient presentation.
pub struct Notifications {
    pub history: Vec<Notice>,
    pending: VecDeque<usize>,
    indexes: Box<Indexes>,
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
            pending: VecDeque::new(),
            indexes: Box::default(),
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
        let event_key = (job, severity, text.clone());
        let duplicate = self.matching_event(&event_key);
        let group = self.toast_group(severity, key);
        let expires = self.clock.saturating_add(self.lifetime());
        let index = if let Some(index) = duplicate {
            let event = &mut self.history[index];
            event.occurrences = event.occurrences.saturating_add(1);
            event.text = text;
            event.presentation = presentation;
            event.group = group;
            event.severity = severity;
            event.created = now;
            event.expires = expires;
            event.updated = self.clock;
            index
        } else {
            self.history.push(Notice {
                presentation,
                group,
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
        self.indexes.events.insert(event_key, index);
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
        let pending = &mut self.pending;
        pending.retain(|id| {
            *id != index
                && (job.is_none() || history[*id].job != job)
                && history[*id].group != history[index].group
        });
        pending.push_back(index);
    }

    fn matching_event(&self, key: &(Option<u64>, Severity, String)) -> Option<usize> {
        self.indexes.events.get(key).copied().filter(|index| {
            key.0.is_some()
                || self.clock.saturating_sub(self.history[*index].updated) <= Duration::from_secs(2)
        })
    }

    fn toast_group(&mut self, severity: Severity, key: String) -> usize {
        let key = (severity, key);
        if let Some(&id) = self.indexes.latest_groups.get(&key)
            && self.clock.saturating_sub(self.indexes.groups[id].updated) <= Duration::from_secs(2)
        {
            self.indexes.groups[id].updated = self.clock;
            self.indexes.groups[id].occurrences =
                self.indexes.groups[id].occurrences.saturating_add(1);
            return id;
        }
        let id = self.indexes.groups.len();
        self.indexes.groups.push(ToastGroup {
            updated: self.clock,
            occurrences: 1,
        });
        self.indexes.latest_groups.insert(key, id);
        id
    }

    fn lifetime(&self) -> Duration {
        Duration::from_secs(u64::from(self.expiry_seconds.clamp(1, 60)))
    }

    /// Advance an injected monotonic clock. Expiry changes presentation only.
    pub fn advance(&mut self, elapsed: Duration) -> bool {
        self.clock = self.clock.max(elapsed);
        let pending = &mut self.pending;
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
        self.pending.clear();
    }

    /// Dismiss one visual group; independently retained events and Jobs survive.
    pub fn dismiss_toast(&mut self, id: usize) {
        self.pending.retain(|event| *event != id);
    }

    /// Retire displaced presentations during update so resizing cannot revive them.
    pub(crate) fn retain_toasts(&mut self, visible: &[usize]) {
        self.pending.retain(|id| visible.contains(id));
    }

    /// Newest presentations first, nearest the configured corner.
    pub fn visible(&self) -> Vec<usize> {
        self.pending
            .iter()
            .rev()
            .copied()
            .filter(|id| self.history[*id].expires > self.clock)
            .collect()
    }

    /// Presentation counts never merge the independently retained Job links.
    pub fn toast_occurrences(&self, id: usize) -> usize {
        self.indexes.groups[self.history[id].group].occurrences
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
    fn indexed_application_groups_extend_at_two_seconds_and_split_afterward() {
        let mut notices = Notifications::default();
        for millis in [0, 2000, 4000] {
            notices.advance(Duration::from_millis(millis));
            notices.notify("same".into(), SystemTime::UNIX_EPOCH, Severity::Warning);
        }
        assert_eq!(notices.history.len(), 1);
        assert_eq!(notices.history[0].occurrences, 3);
        assert_eq!(notices.toast_occurrences(0), 3);
        notices.advance(Duration::from_millis(6001));
        notices.notify("same".into(), SystemTime::UNIX_EPOCH, Severity::Warning);
        assert_eq!(notices.history.len(), 2);
        assert_eq!(notices.toast_occurrences(1), 1);
        assert_eq!(notices.toast_occurrences(0), 3);
        notices.notify("same".into(), SystemTime::UNIX_EPOCH, Severity::Error);
        assert_eq!(notices.visible(), [2, 1, 0]);
    }

    #[test]
    fn repeating_a_job_after_the_group_window_keeps_evidence_but_starts_a_new_count() {
        let mut notices = Notifications::default();
        notices.record(
            "same".into(),
            SystemTime::UNIX_EPOCH,
            Severity::Error,
            Some(7),
        );
        notices.advance(Duration::from_secs(3));
        notices.record(
            "same".into(),
            SystemTime::UNIX_EPOCH,
            Severity::Error,
            Some(7),
        );
        assert_eq!(notices.history.len(), 1);
        assert_eq!(notices.history[0].job, Some(7));
        assert_eq!(notices.history[0].occurrences, 2);
        assert_eq!(notices.toast_occurrences(0), 1);
        assert_eq!(notices.visible(), [0]);
    }

    #[test]
    fn adjacent_two_second_repeats_form_one_transitive_toast_group() {
        let mut notices = Notifications::default();
        for (id, millis) in [(1, 0), (2, 1500), (3, 3000)] {
            notices.advance(Duration::from_millis(millis));
            notices.record(
                "same failure".into(),
                SystemTime::UNIX_EPOCH,
                Severity::Error,
                Some(id),
            );
        }
        assert_eq!(notices.visible(), [2]);
        assert_eq!(notices.toast_occurrences(2), 3);
        assert_eq!(notices.history.len(), 3);
        notices.advance(Duration::from_millis(5001));
        notices.record(
            "same failure".into(),
            SystemTime::UNIX_EPOCH,
            Severity::Error,
            Some(4),
        );
        assert_eq!(notices.visible(), [3, 2]);
        assert_eq!(notices.toast_occurrences(3), 1);
        assert_eq!(notices.toast_occurrences(2), 3);
    }

    #[test]
    fn a_unique_burst_retains_presentations_until_geometry_budgets_them() {
        let mut notices = Notifications::default();
        for index in 0..100 {
            notices.notify(index.to_string(), SystemTime::UNIX_EPOCH, Severity::Info);
        }
        assert_eq!(notices.pending.len(), 100);
        assert_eq!(notices.history.len(), 100);
        assert_eq!(notices.visible(), (0..100).rev().collect::<Vec<_>>());
    }
}
