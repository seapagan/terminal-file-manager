#[cfg(test)]
#[path = "../../tests/policy_choices/revision.rs"]
mod tests;
use super::{App, IgnoreMode};
use crate::{
    input::Action,
    listing::{Grouping, Sort},
};

impl App {
    pub(crate) fn select_policy(&mut self, action: Action, selected: usize) -> bool {
        let pane = &mut self.panes[self.active];
        let changed = match action {
            Action::Sort => select(
                &mut pane.sort.sort,
                &[
                    Sort::Natural,
                    Sort::Name,
                    Sort::Extension,
                    Sort::Size,
                    Sort::Modified,
                    Sort::Created,
                    Sort::Kind,
                    Sort::Version,
                    Sort::Unsorted,
                ],
                selected,
            ),
            Action::Group => select(
                &mut pane.sort.grouping,
                &[Grouping::First, Grouping::Last, Grouping::Mixed],
                selected,
            ),
            Action::Hidden => select(&mut pane.show_hidden, &[false, true], selected),
            Action::Ignore => select(
                &mut pane.ignore,
                &[IgnoreMode::Dim, IgnoreMode::Hide, IgnoreMode::Off],
                selected,
            ),
            _ => false,
        };
        if changed {
            self.policy_changed();
        }
        changed
    }

    pub(crate) fn policy_changed(&mut self) {
        let pane = &mut self.panes[self.active];
        pane.profile_modified = pane.profile.is_some();
        pane.resort();
    }
}

fn select<T: Copy + PartialEq>(current: &mut T, choices: &[T], selected: usize) -> bool {
    let Some(value) = choices.get(selected) else {
        return false;
    };
    if current == value {
        return false;
    }
    *current = *value;
    true
}
