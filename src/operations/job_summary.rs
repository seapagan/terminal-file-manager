use super::{Job, Outcome};

/// Compact final-record presentation. Rebuilt on each finish, including reconciliation.
#[derive(Clone)]
pub struct JobSummary {
    pub(crate) counts: String,
    pub(crate) all_completed: Option<usize>,
    pub(crate) groups: Vec<(Outcome, String, usize)>,
    pub(crate) omitted_groups: usize,
    pub(crate) roots: Vec<Option<Outcome>>,
    pub(crate) reconcilable: bool,
    pub(crate) incident: bool,
}

impl JobSummary {
    pub(crate) fn new(job: &Job) -> Self {
        let record = job.record.as_ref();
        let mut outcomes: std::collections::HashMap<_, _> = job
            .plan
            .operands
            .iter()
            .map(|operand| (operand.path.as_path(), None))
            .collect();
        if let Some(record) = record {
            for entry in &record.entries {
                if let Some(outcome) = outcomes.get_mut(entry.source.as_path()) {
                    *outcome = Some(entry.outcome);
                }
            }
        }
        let roots = job
            .plan
            .operands
            .iter()
            .map(|operand| outcomes[operand.path.as_path()])
            .collect();
        let groups = record.map_or_else(Vec::new, |record| record.failure_groups());
        Self {
            counts: super::counts_summary(job),
            all_completed: record
                .filter(|r| {
                    !r.entries.is_empty()
                        && r.entries.iter().all(|e| e.outcome == Outcome::Completed)
                })
                .map(|r| r.entries.len()),
            // Summary stays compact even with unique errors; Evidence retains every detail.
            omitted_groups: groups.len().saturating_sub(8),
            groups: groups
                .into_iter()
                .take(8)
                .map(|(outcome, detail, count)| (outcome, detail.to_owned(), count))
                .collect(),
            roots,
            reconcilable: record.is_some_and(super::Record::reconcilable),
            incident: record.is_some_and(super::Record::incident),
        }
    }
}

impl Job {
    pub(crate) fn presentation(&self) -> std::borrow::Cow<'_, JobSummary> {
        self.presentation.as_ref().map_or_else(
            || std::borrow::Cow::Owned(JobSummary::new(self)),
            std::borrow::Cow::Borrowed,
        )
    }
}
