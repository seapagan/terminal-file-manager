use crate::app::App;
use ratatui::layout::Rect;

/// File panes, optional metadata and stable bottom controls.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub panes: [Option<Rect>; 2],
    pub sidebar: Option<Rect>,
    pub status: Rect,
    pub footer: Rect,
    pub footer_controls: Vec<super::FooterControl>,
    pub menu: Option<Rect>,
}

/// Width-driven layout never depends on a directory's longest filename.
pub fn geometry(app: &App, area: Rect) -> Geometry {
    let height = area.height.saturating_sub(2);
    let body = Rect::new(area.x, area.y, area.width, height);
    let mut geometry = Geometry {
        panes: [None, None],
        sidebar: None,
        footer_controls: Vec::new(),
        menu: menu_area(app, area),
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
    };
    geometry.footer_controls = super::footer::controls(geometry.footer);
    if app.visible_panes(area.width).len() == 2 {
        let width = body.width / 2;
        geometry.panes = [
            Some(Rect::new(body.x, body.y, width, height)),
            Some(Rect::new(
                body.x + width,
                body.y,
                body.width - width,
                height,
            )),
        ];
    } else if app.sidebar && !app.dual && body.width >= 100 {
        sidebar(&mut geometry, app, body);
    } else {
        geometry.panes[app.active] = Some(body);
    }
    geometry
}

fn menu_area(app: &App, area: Rect) -> Option<Rect> {
    if app.view_menu.is_some() {
        Some(super::view::area(area, crate::input::view::OPTIONS.len()))
    } else if app.profile_menu.is_some() {
        Some(super::view::area(area, app.profile_names.len() + 2))
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
