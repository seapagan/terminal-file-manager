//! Local mutations share frozen operands and one path coordinator.
mod execute;
mod job_summary;
mod locks;
mod model;
pub use job_summary::JobSummary;
mod mounts;
mod mutations;
mod preview;
mod recovery;
pub use model::{
    Dialog, Intent, Job, JobState, Notice, NoticeSeverity, OperationDialog, PendingConflict, State,
};
pub(crate) use model::{counts_summary, summary};
pub use recovery::{Recovery, reconcile};
mod queue;
pub use preview::Preview;
#[cfg(test)]
pub(crate) use queue::tests::Driver as QueueTestDriver;
pub use queue::{Queue, Update};
mod plan;
mod results;
mod safety;
mod staging;
mod transfer;
/// Native Trash boundaries; Linux is restricted to validated same-filesystem rename.
pub mod trash;
pub use execute::execute;
pub use locks::Coordinator;
pub use plan::{Operation, Plan};
pub use results::{Decision, EntryResult, Event, Options, Outcome, Record, Stage, TrashResult};
