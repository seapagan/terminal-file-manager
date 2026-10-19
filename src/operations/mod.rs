//! Local mutations share frozen operands and one path coordinator.
mod execute;
mod locks;
mod plan;
mod results;
mod safety;
mod staging;
mod transfer;
pub use execute::execute;
pub use locks::Coordinator;
pub use plan::{Operation, Plan};
pub use results::{Decision, EntryResult, Event, Options, Outcome, Record, Stage};
