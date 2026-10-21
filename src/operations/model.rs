use super::{Decision, Event, Options, Outcome, Plan, Record};
use std::{collections::VecDeque, path::PathBuf, sync::mpsc::SyncSender, time::SystemTime};

/// Pure UI intents; the browser bridge submits them to the worker coordinator.
pub enum Intent {
    Submit(Plan, Options),
    Preview(Plan),
    Cancel(u64),
    Retry(u64),
    Reconcile(u64),
    SubmitRetry(Plan, Options, u64),
}

pub enum Dialog {
    Operation(Box<OperationDialog>),
    Exit { save: bool },
}

pub struct OperationDialog {
    pub plan: Plan,
    pub pane: usize,
    pub base: PathBuf,
    pub native_target: PathBuf,
    pub text: String,
    pub incoming: String,
    pub edited: bool,
    pub field: usize,
    pub options: Options,
    pub ready: bool,
    pub preview_id: Option<u64>,
    pub track_scope: bool,
    pub retry_of: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Skipped,
    Failed,
    Critical,
    Partial,
    Cancelled,
    Unknown,
}

pub struct Job {
    pub id: u64,
    pub plan: Plan,
    pub options: Options,
    pub state: JobState,
    pub record: Option<Record>,
    pub progress: Option<Event>,
    pub started: SystemTime,
    pub finished: Option<SystemTime>,
    pub retry_of: Option<u64>,
}

impl Job {
    pub fn active(&self) -> bool {
        matches!(self.state, JobState::Queued | JobState::Running)
    }
    /// Retry remains an explicit fresh attempt; the plan revalidates its frozen identities.
    pub fn can_retry(&self) -> bool {
        !self.active()
            && self
                .record
                .as_ref()
                .is_some_and(|record| self.plan.retry(record).is_ok())
    }
    /// Only unknown entries with retained ownership evidence can be reconciled.
    pub fn can_reconcile(&self) -> bool {
        !self.active() && self.record.as_ref().is_some_and(Record::reconcilable)
    }
    pub fn finish(&mut self, record: Record, now: SystemTime) {
        self.finished.get_or_insert(now);
        self.state = finished_state(&record);
        self.record = Some(record);
    }
}

fn finished_state(record: &Record) -> JobState {
    let has = |outcome| record.entries.iter().any(|entry| entry.outcome == outcome);
    for (outcome, state) in [
        (Outcome::SafetyIncident, JobState::Critical),
        (Outcome::OutcomeUnknown, JobState::Unknown),
        (Outcome::Cancelled, JobState::Cancelled),
    ] {
        if has(outcome) {
            return state;
        }
    }
    if has(Outcome::CopiedSourceRetained) {
        return JobState::Partial;
    }
    let completed = has(Outcome::Completed);
    if has(Outcome::Failed) || has(Outcome::Changed) {
        return if completed {
            JobState::Partial
        } else {
            JobState::Failed
        };
    }
    if has(Outcome::Skipped) || record.entries.is_empty() {
        return if completed {
            JobState::Partial
        } else {
            JobState::Skipped
        };
    }
    JobState::Completed
}

pub struct PendingConflict {
    pub id: u64,
    pub event: Event,
    pub reply: SyncSender<Decision>,
    pub all: bool,
    pub choice: usize,
}

pub struct Notice {
    pub text: String,
    pub expires: SystemTime,
    pub warning: bool,
}

/// Session-only operation state. Toast expiry never removes the job's result evidence.
#[derive(Default)]
pub struct State {
    pub dialog: Option<Dialog>,
    pub outbox: Vec<Intent>,
    pub history: Vec<Job>,
    pub conflicts: VecDeque<PendingConflict>,
    pub show_history: bool,
    pub cursor: usize,
    pub scroll: usize,
    pub waiting_exit: Option<bool>,
    pub notices: VecDeque<Notice>,
    pub exit_snapshot: Option<crate::session::NavigationSession>,
}

impl State {
    pub fn active(&self) -> bool {
        self.history.iter().any(Job::active)
    }
    pub fn unresolved(&self) -> bool {
        self.history
            .iter()
            .any(|job| matches!(job.state, JobState::Unknown | JobState::Critical))
    }
    pub fn notify(&mut self, text: String, now: SystemTime, warning: bool) {
        self.notices
            .retain(|notice| notice.text != text && notice.expires > now);
        self.notices.push_back(Notice {
            text,
            expires: now + std::time::Duration::from_secs(5),
            warning,
        });
        while self.notices.len() > 3 {
            self.notices.pop_front();
        }
    }
}

pub(crate) fn summary(job: &Job) -> String {
    let Some(record) = &job.record else {
        return format!("Job {} {:?}: {:?}", job.id, job.plan.operation, job.state);
    };
    if record
        .entries
        .iter()
        .any(|entry| entry.outcome == Outcome::SafetyIncident)
    {
        return format!(
            "Job {}: CRITICAL SAFETY INCIDENT: Windows reported permanent deletion during Trash; stop and report",
            job.id
        );
    }
    let completed = completed(job, record);
    let counts = |outcome| {
        record
            .entries
            .iter()
            .filter(|entry| entry.outcome == outcome)
            .count()
    };
    let failed = record
        .entries
        .iter()
        .filter(|entry| matches!(entry.outcome, Outcome::Failed | Outcome::Changed))
        .count();
    format!(
        "Job {}: {completed}; {} skipped, {failed} failed, {} cancelled, {} source retained, {} unknown",
        job.id,
        counts(Outcome::Skipped),
        counts(Outcome::Cancelled),
        counts(Outcome::CopiedSourceRetained),
        counts(Outcome::OutcomeUnknown) + counts(Outcome::SafetyIncident)
    )
}

fn completed(job: &Job, record: &Record) -> String {
    use super::Operation::*;
    let verb = match job.plan.operation {
        Copy => "copied",
        Move => "moved",
        Mkdir => "created",
        Rename => "renamed",
        Trash => "trashed",
        Delete => "permanently deleted",
        _ => "updated",
    };
    if job.plan.operation == Trash {
        return trash_completed(record);
    }
    if job.plan.operation == Mkdir {
        return format!(
            "{} {verb}",
            noun(record.completed_directories(), "directory", "directories")
        );
    }
    if matches!(job.plan.operation, Rename | Permissions(_) | ReadOnly(_)) {
        return format!(
            "{} {verb}",
            noun(
                record
                    .entries
                    .iter()
                    .filter(|entry| entry.outcome == Outcome::Completed)
                    .count(),
                "entry",
                "entries"
            )
        );
    }
    let mut text = format!("{} {verb}", noun(record.completed_files(), "file", "files"));
    if record.completed_directories() > 0 {
        text.push_str(&format!(
            ", {} {verb}",
            noun(record.completed_directories(), "directory", "directories")
        ));
    }
    text
}

fn noun(number: usize, singular: &str, plural: &str) -> String {
    format!("{number} {}", if number == 1 { singular } else { plural })
}

fn trash_completed(record: &Record) -> String {
    let count = record
        .entries
        .iter()
        .filter(|entry| entry.outcome == Outcome::Completed)
        .count();
    format!(
        "{} trashed (native top-level acknowledgements)",
        noun(count, "entry", "entries")
    )
}
