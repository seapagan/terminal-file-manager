//! Retain accepted worker diagnostics without recording stale results or navigation noise.
use super::App;
use crate::notifications::Severity;

impl App {
    /// Retain rejected operation requests without inventing a filesystem job.
    pub fn reject_operation(&mut self, detail: String) {
        let text = format!("Not started: {detail}");
        self.status = super::Status::JobsDetails(text.clone());
        self.operations.last_error = Some(detail);
        self.notifications.notify(text, self.now, Severity::Error);
    }

    pub(super) fn listing_notices(&self, pane: usize) -> Vec<String> {
        let state = &self.panes[pane];
        state
            .error
            .iter()
            .chain(state.warnings.iter())
            .cloned()
            .collect()
    }

    pub(super) fn retain_listing_notices(&mut self, pane: usize, previous: &[String]) {
        for message in self.listing_notices(pane) {
            if !previous.contains(&message) {
                self.notifications.notify(
                    format!("Pane {}: {message}", pane + 1),
                    self.now,
                    Severity::Warning,
                );
            }
        }
    }
}
