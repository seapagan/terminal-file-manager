//! Local mutations share frozen operands and one path coordinator.
mod execute;
mod locks;
mod model;
mod mounts;
mod mutations;
mod preview;
mod recovery;
pub(crate) use model::summary;
pub use model::{Dialog, Intent, Job, JobState, Notice, OperationDialog, PendingConflict, State};
pub use recovery::{Recovery, reconcile};
mod queue;
pub use preview::Preview;
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
pub use results::{Decision, EntryResult, Event, Options, Outcome, Record, Stage};
