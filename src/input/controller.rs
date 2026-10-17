use super::{Action, Bindings};
use crate::{
    app::{App, IgnoreMode, Request},
    listing::{Grouping, Sort},
    settings::Settings,
    ui::Geometry,
};
use crossterm::event::{
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::{path::PathBuf, time::Duration};

/// Pure input controller; emitted requests are the only route to background I/O.
pub struct Controller {
    pub app: App,
    pub quit: bool,
    pub save_session: bool,
    pub preferences_changed: bool,
    pub preferences: Settings,
    bindings: Bindings,
    filter_before: Option<String>,
    pub(super) last_click: Option<(usize, PathBuf, Duration)>,
    pub(super) dragging_divider: bool,
}

impl Controller {
    /// Own the authoritative app and validated keymap.
    pub fn new(mut app: App, bindings: Bindings, preferences: Settings) -> Self {
        app.help_rows = bindings.help_rows();
        app.metadata_presentation = preferences.metadata_presentation;
        app.profile_names = preferences.profiles.keys().cloned().collect();
        Self {
            app,
            quit: false,
            save_session: false,
            preferences_changed: false,
            preferences,
            bindings,
            filter_before: None,
            last_click: None,
            dragging_divider: false,
        }
    }

    /// Normalize terminal input. Clock/geometry are explicit deterministic inputs.
    pub fn handle(&mut self, event: Event, geometry: &Geometry, now: Duration) -> Vec<Request> {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.quit = true;
                    return Vec::new();
                }
                if self.filter_before.is_some() {
                    self.filter_key(key);
                    return Vec::new();
                }
                if self.app.profile_menu.is_some() {
                    super::profiles::key(self, key);
                    return Vec::new();
                }
                if self.app.view_menu.is_some() {
                    super::view::key(self, key, geometry);
                    return Vec::new();
                }
                self.bindings
                    .action(key)
                    .map_or_else(Vec::new, |action| self.dispatch(action, geometry))
            }
            Event::Paste(text) if self.filter_before.is_some() => {
                self.append_filter(&text);
                Vec::new()
            }
            Event::Mouse(event) => self.mouse(event, geometry, now),
            Event::Resize(width, height) => {
                super::scroll::resize(self, width, height);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Keyboard and clickable controls share this dispatcher and availability policy.
    pub fn dispatch(&mut self, action: Action, geometry: &Geometry) -> Vec<Request> {
        use Action::*;
        let action = self.preferred_action(action);
        if self.app.help
            && !matches!(
                action,
                Up | Down | Home | End | PageUp | PageDown | Help | Cancel | Quit | SaveQuit
            )
        {
            return Vec::new();
        }
        if let Some(reason) = unavailable(action) {
            self.app.status = reason.into();
            return Vec::new();
        }
        match action {
            SplitLeft => super::divider::adjust(self, self.app.split_percent.saturating_sub(5)),
            SplitRight => super::divider::adjust(self, self.app.split_percent.saturating_add(5)),
            ViewProfiles => super::profiles::open(self),
            ViewOptions => {
                self.app.view_menu = if self.app.view_menu.is_some() {
                    None
                } else {
                    Some(0)
                };
                self.app.popup = false;
                self.app.help = false;
            }
            HumanSizes | FuzzyTimestamps | Permissions | ColumnSize | ColumnModified
            | ColumnAttributes | ColumnOwner | OwnerDisplay | MetadataPresentation => {
                super::view::formatting(self, action)
            }
            Up | Down | Home | End | PageUp | PageDown => self.movement(action, geometry),
            Enter | Parent | Back | Forward | Refresh => return self.navigate(action),
            Mark | MarkNext | SelectAll | ClearVisible => self.selection(action, geometry),
            Hidden | Ignore | Sort | Reverse | Group => self.policy(action),
            TogglePanes | Maximise | FocusPane | ToggleSidebar | Metadata | Help => {
                self.layout(action, geometry)
            }
            Quit | SaveQuit => self.exit(action),
            Filter => {
                self.filter_before = Some(self.app.panes[self.app.active].filter.clone());
                self.app.status = "Filter: type to narrow; Enter accepts, Esc restores".into();
            }
            Count => return self.count(),
            Cancel => self.cancel(),
            _ => {}
        }
        Vec::new()
    }

    fn exit(&mut self, action: Action) {
        self.save_session = action == Action::SaveQuit;
        self.quit = true;
    }

    fn preferred_action(&self, action: Action) -> Action {
        if action != Action::Info {
            return action;
        }
        match self.preferences.metadata_presentation {
            crate::settings::MetadataPresentation::Sidebar => Action::ToggleSidebar,
            crate::settings::MetadataPresentation::Popup => Action::Metadata,
        }
    }

    fn movement(&mut self, action: Action, geometry: &Geometry) {
        let height = page_height(geometry, self.app.active);
        if self.app.help || self.app.popup {
            let delta = match action {
                Action::Up => -1,
                Action::Home => -i16::MAX,
                Action::End => i16::MAX,
                Action::PageUp => -(height as i16),
                Action::PageDown => height as i16,
                _ => 1,
            };
            self.app.overlay_scroll = self
                .app
                .overlay_scroll
                .min(geometry.overlay_scroll_limit)
                .saturating_add_signed(delta)
                .min(geometry.overlay_scroll_limit);
            return;
        }
        let pane = &mut self.app.panes[self.app.active];
        let delta = match action {
            Action::Up => -1,
            Action::Home => -isize::MAX,
            Action::End => isize::MAX,
            Action::PageUp => -(height as isize),
            Action::PageDown => height as isize,
            _ => 1,
        };
        pane.move_cursor(delta, height);
    }

    fn navigate(&mut self, action: Action) -> Vec<Request> {
        let id = self.app.active;
        if action == Action::Back {
            return self.app.history(false).into_iter().collect();
        }
        if action == Action::Forward {
            return self.app.history(true).into_iter().collect();
        }
        let pane = &self.app.panes[id];
        let path = match action {
            Action::Enter => {
                let Some(entry) = pane.current() else {
                    self.app.status = "No visible entry".into();
                    return Vec::new();
                };
                if !entry.enterable {
                    self.app.status =
                        "Viewer unavailable until M6; files are never launched".into();
                    return Vec::new();
                }
                entry.path.clone()
            }
            Action::Parent => {
                let Some(parent) = pane.path.parent() else {
                    self.app.status = "Filesystem root".into();
                    return Vec::new();
                };
                parent.to_path_buf()
            }
            _ => pane.path.clone(),
        };
        vec![self.app.request_listing(id, path)]
    }

    fn selection(&mut self, action: Action, geometry: &Geometry) {
        let height = page_height(geometry, self.app.active);
        let pane = &mut self.app.panes[self.app.active];
        match action {
            Action::Mark => pane.toggle_mark(),
            Action::MarkNext => {
                pane.toggle_mark();
                pane.move_cursor(1, height);
            }
            Action::SelectAll => pane.select_all(),
            Action::ClearVisible => pane.clear_visible(),
            _ => {}
        }
    }

    fn policy(&mut self, action: Action) {
        let pane = &mut self.app.panes[self.app.active];
        match action {
            Action::Hidden => pane.show_hidden = !pane.show_hidden,
            Action::Ignore => {
                pane.ignore = match pane.ignore {
                    IgnoreMode::Dim => IgnoreMode::Hide,
                    IgnoreMode::Hide => IgnoreMode::Off,
                    IgnoreMode::Off => IgnoreMode::Dim,
                }
            }
            Action::Sort => pane.sort.sort = next_sort(pane.sort.sort),
            Action::Reverse => pane.sort.reverse = !pane.sort.reverse,
            Action::Group => {
                pane.sort.grouping = match pane.sort.grouping {
                    Grouping::First => Grouping::Last,
                    Grouping::Last => Grouping::Mixed,
                    Grouping::Mixed => Grouping::First,
                }
            }
            _ => {}
        }
        pane.profile_modified = pane.profile.is_some();
        pane.resort();
        self.preferences.capture(&self.app, action);
        self.preferences_changed = true;
    }

    fn layout(&mut self, action: Action, geometry: &Geometry) {
        match action {
            Action::TogglePanes => {
                self.app.dual = !self.app.dual;
                self.app.maximized = false;
                self.dragging_divider = false;
                self.app.status = if self.app.dual {
                    "Two-pane layout"
                } else {
                    "Single-pane layout"
                }
                .into();
            }
            Action::Maximise if self.app.dual => {
                self.app.maximized = !self.app.maximized;
                self.dragging_divider = false;
                self.app.status = if self.app.maximized {
                    "Active pane maximised — Alt+M restores split"
                } else {
                    "Split restored"
                }
                .into();
            }
            Action::FocusPane if geometry.panes.iter().all(Option::is_some) => {
                self.app.active = 1 - self.app.active;
                self.app.sidebar = self.app.panes[self.app.active].sidebar;
            }
            Action::ToggleSidebar => {
                self.app.sidebar = !self.app.sidebar;
                let pane = &mut self.app.panes[self.app.active];
                pane.sidebar = self.app.sidebar;
                pane.profile_modified = pane.profile.is_some();
                self.preferences.capture(&self.app, action);
                self.preferences_changed = true;
            }
            Action::Metadata => {
                self.app.popup = !self.app.popup;
                self.app.overlay_scroll = 0;
            }
            Action::Help => {
                self.app.help = !self.app.help;
                self.app.overlay_scroll = 0;
            }
            _ => {}
        }
    }

    fn count(&mut self) -> Vec<Request> {
        let mut requests: Vec<_> = self
            .app
            .request_metadata(self.app.active)
            .into_iter()
            .collect();
        if let Some(request) = self.app.request_totals() {
            requests.push(request);
            self.app.popup = true;
        } else {
            self.app.status = "Directory totals require a visible directory".into();
        }
        requests
    }

    fn cancel(&mut self) {
        self.app.cancel_totals();
        if self.app.help || self.app.popup {
            self.app.help = false;
            self.app.popup = false;
        } else {
            self.app.panes[self.app.active].set_filter(String::new());
        }
        self.app.overlay_scroll = 0;
    }

    fn filter_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                let before = self.filter_before.take().unwrap_or_default();
                self.app.panes[self.app.active].set_filter(before);
                self.app.status = "Filter cancelled".into();
            }
            KeyCode::Enter => {
                self.filter_before = None;
                self.app.status = "Filter applied".into();
            }
            KeyCode::Backspace => {
                let mut text = self.app.panes[self.app.active].filter.clone();
                text.pop();
                self.app.panes[self.app.active].set_filter(text);
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.append_filter(&c.to_string())
            }
            _ => {}
        }
    }

    fn append_filter(&mut self, text: &str) {
        let mut query = self.app.panes[self.app.active].filter.clone();
        for c in text.chars().filter(|c| !c.is_control()) {
            if query.len() + c.len_utf8() > 4096 {
                break;
            }
            query.push(c);
        }
        self.app.panes[self.app.active].set_filter(query);
    }

    fn overlay_mouse(&mut self, event: MouseEvent, geometry: &Geometry) -> bool {
        if self.app.profile_menu.is_some() {
            if let Some(area) = geometry.menu {
                super::profiles::mouse(self, event, area);
            }
            return true;
        }
        if self.app.view_menu.is_some() {
            super::view::mouse(self, event, geometry);
            return true;
        }
        if self.app.help || self.app.popup {
            super::scroll::wheel(
                &mut self.app.overlay_scroll,
                event.kind,
                geometry.overlay_scroll_limit,
            );
            return true;
        }
        if geometry
            .sidebar
            .is_some_and(|area| area.contains((event.column, event.row).into()))
        {
            super::scroll::wheel(
                &mut self.app.metadata_scroll,
                event.kind,
                geometry.metadata_scroll_limit,
            );
            return true;
        }
        false
    }

    fn mouse(&mut self, event: MouseEvent, geometry: &Geometry, now: Duration) -> Vec<Request> {
        if self.overlay_mouse(event, geometry) {
            return Vec::new();
        }
        if super::divider::mouse(self, event, geometry) {
            return Vec::new();
        }
        if geometry.footer.contains((event.column, event.row).into())
            && event.kind == MouseEventKind::Down(MouseButton::Left)
        {
            let Some(action) = geometry.footer_controls.iter().find_map(|control| {
                control
                    .area
                    .contains((event.column, event.row).into())
                    .then_some(control.action)
            }) else {
                return Vec::new();
            };
            return self.dispatch(action, geometry);
        }
        let Some(id) = geometry.panes.iter().position(|area| {
            area.is_some_and(|area| area.contains((event.column, event.row).into()))
        }) else {
            return Vec::new();
        };
        let height = page_height(geometry, id);
        match event.kind {
            MouseEventKind::ScrollDown => self.app.panes[id].move_cursor(1, height),
            MouseEventKind::ScrollUp => self.app.panes[id].move_cursor(-1, height),
            MouseEventKind::Down(MouseButton::Left) => {
                return self.click(id, event, geometry, now);
            }
            _ => {}
        }
        Vec::new()
    }
}

pub(super) fn page_height(geometry: &Geometry, pane: usize) -> usize {
    geometry.panes[pane].map_or(1, |area| usize::from(area.height.saturating_sub(5)).max(1))
}

fn next_sort(sort: Sort) -> Sort {
    match sort {
        Sort::Natural => Sort::Name,
        Sort::Name => Sort::Extension,
        Sort::Extension => Sort::Size,
        Sort::Size => Sort::Modified,
        Sort::Modified => Sort::Created,
        Sort::Created => Sort::Kind,
        Sort::Kind => Sort::Version,
        Sort::Version => Sort::Unsorted,
        Sort::Unsorted => Sort::Natural,
    }
}

fn unavailable(action: Action) -> Option<&'static str> {
    match action {
        Action::Viewer | Action::Editor => {
            Some("Viewer/editor unavailable until M6; files are never launched")
        }
        Action::Copy | Action::Move | Action::Mkdir | Action::Trash | Action::Delete => {
            Some("File mutations unavailable until M2 — M1 is read-only")
        }
        Action::Path | Action::Zoxide | Action::OppositeZoxide | Action::ExitDirectory => {
            Some("Navigation integration/shell handoff unavailable until M5")
        }
        _ => None,
    }
}
