//! Local mutations share frozen operands and one path coordinator.
mod locks;
mod plan;
pub use locks::Coordinator;
pub use plan::{Operation, Plan};
