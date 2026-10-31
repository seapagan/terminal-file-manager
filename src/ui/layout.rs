use crate::app::App;
use ratatui::layout::Rect;

/// File panes, optional metadata and stable bottom controls.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub context_help: Option<super::ContextLayout>,
    pub context_control: Option<Rect>,
    pub modal_close: Option<Rect>,
    pub help: Option<super::help::HelpLayout>,
    pub pattern: Option<super::PatternLayout>,
    pub(super) operation_layout: Option<super::operations::Layout>,
    pub operation_controls: Vec<super::OperationControl>,
    pub operation_scroll_limit: usize,
    pub panes: [Option<Rect>; 2],
    pub listing_rows: [usize; 2],
    pub selection_counts: [crate::app::SelectionCounts; 2],
    pub divider: Option<Rect>,
    pub sidebar: Option<Rect>,
    pub status: Rect,
    pub footer: Rect,
    pub footer_controls: Vec<super::FooterControl>,
    pub menu: Option<Rect>,
    pub diagnostics: Option<Rect>,
    pub diagnostics_scroll_limit: u16,
    pub overlay_scroll_limit: u16,
    pub metadata_scroll_limit: u16,
    pub metadata_controls: Vec<super::MetadataControl>,
}

impl Geometry {
    /// Eligibility from the same measured controls used for rendering and mouse input.
    pub fn operation_enabled(&self, key: crossterm::event::KeyCode) -> bool {
        self.operation_layout.as_ref().is_some_and(|layout| {
            layout
                .shell
                .actions
                .iter()
                .any(|region| region.action.enabled && region.action.key == key)
        })
    }

    pub(crate) fn operation_scroll_contains(&self, column: u16, row: u16) -> bool {
        self.operation_layout
            .as_ref()
            .is_some_and(|layout| layout.body.contains((column, row).into()))
    }

    /// Modal actions use the shared shell's enabled hit test; field/job targets use
    /// the visible rows measured in that same layout.
    pub fn operation_target(&self, column: u16, row: u16) -> Option<super::OperationTarget> {
        self.operation_layout
            .as_ref()
            .and_then(|layout| layout.shell.hit(column, row).copied())
            .or_else(|| {
                self.operation_controls
                    .iter()
                    .find(|control| {
                        !matches!(control.target, super::OperationTarget::Key(_))
                            && control.area.contains((column, row).into())
                    })
                    .map(|control| control.target)
            })
    }
}

/// Width-driven layout never depends on a directory's longest filename.
pub fn geometry(app: &App, area: Rect) -> Geometry {
    let height = area.height.saturating_sub(2);
    let body = Rect::new(area.x, area.y, area.width, height);
    let mut geometry = initial_geometry(app, area, height);
    geometry.footer_controls = super::footer::controls(app, geometry.footer);
    if app.visible_panes(area.width).len() == 2 {
        let width = ((u32::from(body.width) * u32::from(app.split_percent.clamp(20, 80)) / 100)
            as u16)
            .clamp(24, body.width.saturating_sub(24));
        geometry.divider = Some(Rect::new(body.x + width - 1, body.y, 2, height));
        geometry.panes = [
            Some(Rect::new(body.x, body.y, width, height)),
            Some(Rect::new(
                body.x + width,
                body.y,
                body.width - width,
                height,
            )),
        ];
    } else if app.sidebar_visible(body.width) {
        sidebar(&mut geometry, app, body);
    } else {
        geometry.panes[app.active] = Some(body);
    }
    scroll_limits(app, &mut geometry, area);
    geometry.modal_close = close_control(app, area, &geometry);
    geometry
}

fn initial_geometry(app: &App, area: Rect, height: u16) -> Geometry {
    let (operation_controls, operation_scroll_limit, operation_layout) =
        super::operations::geometry(app, area);
    let pattern = super::pattern::layout(app, area);
    let context_control = context_control(app, area, operation_layout.as_ref(), pattern.as_ref());
    Geometry {
        context_help: super::help::context::layout(app, area),
        context_control,
        modal_close: None,
        help: super::help::layout(app, area),
        pattern,
        operation_layout,
        operation_controls,
        operation_scroll_limit,
        panes: [None, None],
        listing_rows: [0, 0],
        selection_counts: [Default::default(); 2],
        overlay_scroll_limit: 0,
        metadata_scroll_limit: 0,
        metadata_controls: Vec::new(),
        divider: None,
        sidebar: None,
        footer_controls: Vec::new(),
        menu: menu_area(app, area),
        diagnostics: app.diagnostics.then(|| super::diagnostics::area(area)),
        diagnostics_scroll_limit: 0,
        status: Rect::new(
            area.x,
            area.y + height,
            area.width,
            u16::from(area.height >= 2),
        ),
        footer: Rect::new(
            area.x,
            area.bottom().saturating_sub(1),
            area.width,
            u16::from(area.height >= 1),
        ),
    }
}

fn menu_area(app: &App, area: Rect) -> Option<Rect> {
    if app.view_menu.is_some() {
        Some(super::view::area(area, crate::input::view::OPTIONS.len()))
    } else if app.profile_menu.is_some() {
        Some(super::profiles::area(app, area))
    } else {
        None
    }
}

fn sidebar(geometry: &mut Geometry, app: &App, body: Rect) {
    let width = (body.width / 3).min(48);
    let info_x = if app.sidebar_left {
        body.x
    } else {
        body.right() - width
    };
    let pane_x = if app.sidebar_left {
        body.x + width
    } else {
        body.x
    };
    geometry.sidebar = Some(Rect::new(info_x, body.y, width, body.height));
    geometry.panes[app.active] = Some(Rect::new(pane_x, body.y, body.width - width, body.height));
}

fn scroll_limits(app: &App, geometry: &mut Geometry, area: Rect) {
    for (id, area) in geometry.panes.iter().enumerate() {
        if let Some(area) = area {
            let counts = app.panes[id].selection_counts();
            geometry.selection_counts[id] = counts;
            geometry.listing_rows[id] =
                super::summary::row_count(app.panes[id].visible.len(), counts, *area);
        }
    }
    geometry.overlay_scroll_limit = if app.help {
        geometry
            .help
            .as_ref()
            .map_or(0, |layout| layout.scroll_limit)
    } else if app.popup {
        super::metadata::scroll_limit(app, area, true)
    } else {
        0
    };
    geometry.metadata_scroll_limit = geometry
        .sidebar
        .map_or(0, |area| super::metadata::scroll_limit(app, area, false));
    if !app.help && geometry.menu.is_none() {
        geometry.metadata_controls = if app.popup {
            super::metadata::controls(app, area, true)
        } else {
            geometry
                .sidebar
                .map_or_else(Vec::new, |area| super::metadata::controls(app, area, false))
        };
    }
}

fn context_control(
    app: &App,
    area: Rect,
    operation: Option<&super::operations::Layout>,
    pattern: Option<&super::PatternLayout>,
) -> Option<Rect> {
    crate::input::HelpTopic::current(app)?;
    if app.popup {
        return super::metadata::help_control(app, area);
    }
    if let Some(layout) = operation {
        return layout.shell.help;
    }
    pattern.and_then(|layout| layout.shell.help)
}

fn close_control(app: &App, area: Rect, geometry: &Geometry) -> Option<Rect> {
    if let Some(layout) = &geometry.context_help {
        return layout.shell.close;
    }
    if let Some(layout) = &geometry.operation_layout {
        return layout.shell.close;
    }
    if let Some(layout) = &geometry.pattern {
        return layout.shell.close;
    }
    if app.diagnostics || geometry.menu.is_some() {
        return None;
    }
    if let Some(layout) = &geometry.help {
        return layout
            .picker
            .as_ref()
            .map_or(layout.close, |picker| picker.shell.close);
    }
    app.popup
        .then(|| super::metadata::close_control(app, area))
        .flatten()
}
