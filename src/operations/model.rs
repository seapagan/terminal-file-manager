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
    IncidentExit { save: bool, typed: String },
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
    pub error: Option<String>,
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
    /// Cached by `finish`; rendering never clones a plan or scans recursive results.
    pub retry_eligible: bool,
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
        !matches!(
            self.state,
            JobState::Queued | JobState::Running | JobState::Unknown | JobState::Critical
        ) && self.retry_eligible
    }
    /// Only unknown entries with retained ownership evidence can be reconciled.
    pub fn can_reconcile(&self) -> bool {
        !self.active() && self.record.as_ref().is_some_and(Record::reconcilable)
    }
    pub fn finish(&mut self, record: Record, now: SystemTime) {
        self.retry_eligible = self.plan.retry_eligibility(&record).is_ok();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeSeverity {
    Success,
    Warning,
    Error,
}

pub struct Notice {
    pub text: String,
    pub expires: SystemTime,
    pub severity: NoticeSeverity,
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
    pub last_error: Option<String>,
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
    /// Irreversible native incidents retain their records until actual exit.
    pub fn incidents(&self) -> bool {
        self.history
            .iter()
            .any(|job| job.record.as_ref().is_some_and(Record::incident))
    }
    /// Ordinary uncertainty never receives the incident acknowledgement bypass.
    pub fn unknown(&self) -> bool {
        self.history
            .iter()
            .filter_map(|job| job.record.as_ref())
            .any(|record| {
                record
                    .entries
                    .iter()
                    .any(|entry| entry.outcome == Outcome::OutcomeUnknown)
            })
    }
    /// A rejected request has no worker job; preserve its evidence in the existing Jobs view.
    pub fn fail(&mut self, detail: String, now: SystemTime) {
        self.last_error = Some(detail);
        self.notify("Operation not started".into(), now, NoticeSeverity::Error);
    }
    pub fn notify(&mut self, text: String, now: SystemTime, severity: NoticeSeverity) {
        self.notices
            .retain(|notice| notice.text != text && notice.expires > now);
        self.notices.push_back(Notice {
            text,
            expires: now + std::time::Duration::from_secs(5),
            severity,
        });
        while self.notices.len() > 2 {
            self.notices.pop_front();
        }
    }
}

pub(crate) fn summary(job: &Job) -> String {
    let Some(record) = &job.record else {
        return format!(
            "Job {}: {}",
            job.id,
            if job.state == JobState::Queued {
                "queued"
            } else {
                "running"
            }
        );
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
    let mut summary = format!("Job {}: {completed}", job.id);
    for (outcome, label) in [
        (Outcome::Skipped, "skipped"),
        (Outcome::Failed, "failed"),
        (Outcome::Changed, "changed"),
        (Outcome::Cancelled, "cancelled"),
        (Outcome::CopiedSourceRetained, "source retained"),
        (Outcome::OutcomeUnknown, "unknown"),
    ] {
        let count = record
            .entries
            .iter()
            .filter(|entry| entry.outcome == outcome)
            .count();
        if count > 0 {
            summary.push_str(&format!("; {count} {label}"));
        }
    }
    summary
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
