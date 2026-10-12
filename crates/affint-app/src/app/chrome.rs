//! Reference-sized native chrome and independently drawn monochrome symbols.
//! Document commands and file/render ownership stay with the application.
use super::*;

pub(super) const TRAY: f64 = 40.;
pub(super) const STUDIO: f64 = 265.;
pub(super) const RULER: f64 = 20.;
pub(super) const ROW: f64 = 32.;
const REMOVE_PROPERTIES: &str = "remove-properties";
const REMOVE_FOOTER: &str = "remove-footer";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Persona {
    Vector,
    Pixel,
    Layout,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ColorTab {
    Color,
    Swatches,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LowerTab {
    Transform,
    History,
    Pages,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LayerTab {
    Layers,
    Effects,
}
pub(super) struct ChromeState {
    pub persona: Persona,
    pub color_tab: ColorTab,
    pub lower_tab: LowerTab,
    pub layer_tab: LayerTab,
    pub wheel_drag: Option<WheelPart>,
    pub color_hue: f64,
    pub opacity_dispatch: OpacityDispatch,
}
impl Default for ChromeState {
    fn default() -> Self {
        Self {
            persona: Persona::Vector,
            color_tab: ColorTab::Color,
            lower_tab: LowerTab::Transform,
            layer_tab: LayerTab::Layers,
            wheel_drag: None,
            color_hue: 0.,
            opacity_dispatch: OpacityDispatch::default(),
        }
    }
}

pub(super) fn grey(value: u8) -> mui::scene::Color {
    color([f32::from(value) / 255.; 3])
}
pub(super) fn divider() -> El {
    leaf(1., 1.).width(pct(100.)).fill(grey(13))
}
pub(super) fn separator() -> El {
    row([leaf(1., 24.).fill(grey(36))])
        .width(13.)
        .align(Align::Center)
        .pad_xy(6., 0.)
}
pub(super) fn tab(id: &str, title: &str, selected: bool, disabled: bool) -> El {
    row([label_text(title)
        .text_size(11.)
        .fill(grey(if disabled { 147 } else { 242 }))])
    .pad_xy(5., 0.)
    .height(23.)
    .align(Align::Center)
    .id(id)
    .role(Kind::Button)
    .label(title)
    .focusable()
    .disabled(disabled)
    .on(State::Focus, |style| style.border(color(ACCENT), 1.))
    .on(State::Hover, |style| style.fill(grey(70)))
    .fill(grey(if selected { 54 } else { 31 }))
    .radius(5.)
}

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Home,
    Move,
    Node,
    Pen,
    Brush,
    Rectangle,
    Ellipse,
    Crop,
    Hand,
    Zoom,
    Text,
    Artboard,
    Undo,
    Redo,
    Save,
    Open,
    Export,
    Fit,
    Actual,
    Eye,
    Up,
    Down,
    Delete,
    Mask,
    Effects,
    Group,
    Pixel,
    Adjust,
    More,
    Lock,
}
pub(super) fn symbol(icon: Icon, size: f64) -> El {
    canvas(move |area| {
        let scale = size / 18.;
        let x = (area.width - size) / 2.;
        let y = (area.height - size) / 2.;
        let line = |points: &[(f64, f64)], closed| {
            Draw::stroke(
                Path::polyline(
                    points
                        .iter()
                        .map(|(px, py)| Point::new(x + px * scale, y + py * scale)),
                    closed,
                ),
                color(INK),
                1. * scale,
            )
        };
        let circle = |cx: f64, cy: f64, radius: f64| {
            let points: Vec<_> = (0..24)
                .map(|i| {
                    let angle = f64::from(i) * std::f64::consts::TAU / 24.;
                    (cx + angle.cos() * radius, cy + angle.sin() * radius)
                })
                .collect();
            line(&points, true)
        };
        match icon {
            Icon::Home => vec![
                line(&[(2., 8.), (9., 2.), (16., 8.)], false),
                line(&[(4., 7.), (4., 16.), (14., 16.), (14., 7.)], false),
                line(&[(7., 16.), (7., 11.), (11., 11.), (11., 16.)], false),
            ],
            Icon::Move => vec![line(
                &[
                    (4., 2.),
                    (4., 15.),
                    (7., 11.),
                    (11., 17.),
                    (13., 15.),
                    (9., 10.),
                    (15., 10.),
                ],
                true,
            )],
            Icon::Node => vec![
                line(&[(3., 3.), (15., 5.), (8., 15.)], true),
                line(&[(1., 1.), (5., 1.), (5., 5.), (1., 5.)], true),
                line(&[(13., 3.), (17., 3.), (17., 7.), (13., 7.)], true),
                line(&[(6., 13.), (10., 13.), (10., 17.), (6., 17.)], true),
            ],
            Icon::Pen => vec![
                line(
                    &[
                        (3., 15.),
                        (6., 5.),
                        (12., 2.),
                        (16., 7.),
                        (13., 13.),
                        (3., 15.),
                    ],
                    true,
                ),
                line(&[(3., 15.), (10., 8.)], false),
                circle(11., 7., 1.),
            ],
            Icon::Brush => vec![
                line(&[(5., 11.), (13., 2.), (16., 5.), (8., 13.)], true),
                line(
                    &[(5., 11.), (2., 12.), (2., 16.), (6., 15.), (8., 13.)],
                    false,
                ),
            ],
            Icon::Rectangle => vec![line(&[(2., 3.), (16., 3.), (16., 15.), (2., 15.)], true)],
            Icon::Ellipse => vec![circle(9., 9., 7.)],
            Icon::Crop => vec![
                line(&[(5., 1.), (5., 13.), (17., 13.)], false),
                line(&[(1., 5.), (13., 5.), (13., 17.)], false),
            ],
            Icon::Hand => vec![line(
                &[
                    (5., 11.),
                    (5., 5.),
                    (7., 5.),
                    (7., 10.),
                    (7., 2.),
                    (9., 2.),
                    (9., 10.),
                    (9., 3.),
                    (11., 3.),
                    (11., 10.),
                    (11., 5.),
                    (13., 5.),
                    (13., 12.),
                    (11., 16.),
                    (7., 16.),
                    (2., 11.),
                    (3., 9.),
                    (5., 11.),
                ],
                false,
            )],
            Icon::Zoom => vec![circle(7., 7., 5.), line(&[(11., 11.), (17., 17.)], false)],
            Icon::Text => vec![
                line(&[(2., 3.), (16., 3.), (16., 6.)], false),
                line(&[(2., 6.), (2., 3.)], false),
                line(&[(9., 3.), (9., 16.)], false),
                line(&[(6., 16.), (12., 16.)], false),
            ],
            Icon::Artboard => vec![
                line(&[(4., 2.), (4., 16.)], false),
                line(&[(2., 4.), (16., 4.)], false),
                line(&[(14., 2.), (14., 16.)], false),
                line(&[(2., 14.), (16., 14.)], false),
            ],
            Icon::Undo | Icon::Redo => {
                let flip = if matches!(icon, Icon::Redo) { -1. } else { 1. };
                let points: Vec<_> = [
                    (5., 4.),
                    (2., 7.),
                    (5., 10.),
                    (2., 7.),
                    (11., 7.),
                    (15., 9.),
                    (15., 13.),
                    (12., 15.),
                    (7., 15.),
                ]
                .into_iter()
                .map(|(px, py)| (9. + (px - 9.) * flip, py))
                .collect();
                vec![line(&points, false)]
            }
            Icon::Save => vec![
                line(
                    &[(3., 2.), (13., 2.), (16., 5.), (16., 16.), (3., 16.)],
                    true,
                ),
                line(&[(6., 2.), (6., 7.), (12., 7.), (12., 2.)], false),
                line(&[(6., 16.), (6., 11.), (13., 11.), (13., 16.)], false),
            ],
            Icon::Open => vec![
                line(&[(2., 14.), (2., 4.), (7., 4.), (9., 6.), (15., 6.)], false),
                line(&[(2., 14.), (5., 8.), (17., 8.), (14., 14.)], true),
            ],
            Icon::Export => vec![
                line(&[(3., 7.), (3., 16.), (15., 16.), (15., 7.)], false),
                line(&[(9., 12.), (9., 2.), (5., 6.), (9., 2.), (13., 6.)], false),
            ],
            Icon::Fit => vec![
                line(&[(2., 7.), (2., 2.), (7., 2.)], false),
                line(&[(11., 2.), (16., 2.), (16., 7.)], false),
                line(&[(2., 11.), (2., 16.), (7., 16.)], false),
                line(&[(11., 16.), (16., 16.), (16., 11.)], false),
            ],
            Icon::Actual => vec![
                line(&[(4., 5.), (6., 3.), (6., 15.)], false),
                line(&[(3., 15.), (9., 15.)], false),
                line(&[(12., 3.), (12., 15.)], false),
            ],
            Icon::Eye => vec![
                line(
                    &[
                        (1., 9.),
                        (5., 5.),
                        (9., 4.),
                        (13., 5.),
                        (17., 9.),
                        (13., 13.),
                        (9., 14.),
                        (5., 13.),
                    ],
                    true,
                ),
                circle(9., 9., 2.),
            ],
            Icon::Up => vec![line(&[(3., 11.), (9., 5.), (15., 11.)], false)],
            Icon::Down => vec![line(&[(3., 7.), (9., 13.), (15., 7.)], false)],
            Icon::Delete => vec![
                line(&[(3., 5.), (15., 5.)], false),
                line(&[(5., 5.), (6., 16.), (12., 16.), (13., 5.)], true),
                line(&[(7., 5.), (7., 2.), (11., 2.), (11., 5.)], false),
            ],
            Icon::Mask => vec![
                line(&[(2., 3.), (16., 3.), (16., 15.), (2., 15.)], true),
                circle(9., 9., 4.),
            ],
            Icon::Effects => vec![
                line(&[(11., 2.), (7., 2.), (7., 16.), (4., 16.)], false),
                line(&[(4., 8.), (11., 8.)], false),
                line(&[(11., 10.), (16., 16.), (16., 10.), (11., 16.)], false),
            ],
            Icon::Group => vec![line(
                &[
                    (2., 5.),
                    (7., 5.),
                    (9., 7.),
                    (16., 7.),
                    (16., 15.),
                    (2., 15.),
                ],
                true,
            )],
            Icon::Pixel => vec![
                line(&[(3., 3.), (15., 3.), (15., 15.), (3., 15.)], true),
                line(&[(9., 3.), (9., 15.)], false),
                line(&[(3., 9.), (15., 9.)], false),
            ],
            Icon::Adjust => vec![circle(9., 9., 7.), line(&[(9., 2.), (9., 16.)], false)],
            Icon::More => vec![
                circle(4., 9., 0.8),
                circle(9., 9., 0.8),
                circle(14., 9., 0.8),
            ],
            Icon::Lock => vec![
                line(&[(4., 8.), (14., 8.), (14., 16.), (4., 16.)], true),
                line(&[(6., 8.), (6., 3.), (12., 3.), (12., 8.)], false),
            ],
        }
    })
    .no_fill()
}
pub(super) fn icon_button(
    id: &str,
    title: &str,
    icon: Icon,
    disabled: bool,
    selected: bool,
    cell: f64,
) -> El {
    symbol(icon, if cell <= 20. { 16. } else { 18. })
        .size(cell, cell)
        .id(id)
        .label(title)
        .role(Kind::Button)
        .focusable()
        .disabled(disabled)
        .fill(grey(if selected { 30 } else { 64 }))
        .radius(8.)
        .border(grey(if selected { 102 } else { 64 }), 1.)
        .on(State::Hover, |style| style.fill(grey(46)))
        .on(State::Focus, |style| style.border(color(ACCENT), 1.))
        .tip(title)
}
fn text_button(id: &str, title: &str, disabled: bool) -> El {
    row([label_text(title)])
        .id(id)
        .role(Kind::Button)
        .label(title)
        .focusable()
        .height(22.)
        .pad_xy(7., 0.)
        .align(Align::Center)
        .radius(2.)
        .disabled(disabled)
        .fill(grey(79))
        .on(State::Hover, |style| style.fill(grey(88)))
        .on(State::Focus, |style| style.border(color(ACCENT), 1.))
}
fn queue_command(
    ui: &Ui,
    commands: &mut Vec<Command>,
    id: &str,
    disabled: bool,
    command: Option<Command>,
) {
    if !disabled
        && activated(ui, id)
        && let Some(command) = command
    {
        commands.push(command);
    }
}
pub(super) fn unavailable(id: &str, title: &str, icon: Icon, cell: f64) -> El {
    icon_button(
        id,
        &format!("{title} · not implemented"),
        icon,
        true,
        false,
        cell,
    )
}
pub(super) fn readout(title: &str, value: String, width: f64) -> El {
    row([
        muted(title),
        row([label_text(value).grow(1.).clip()])
            .grow(1.)
            .pad_xy(5., 3.)
            .fill(grey(31))
            .radius(2.),
    ])
    .width(width)
    .gap(4.)
    .align(Align::Center)
}
pub(super) fn opacity(ui: &Ui, value: &mut f64, disabled: bool) -> (El, bool) {
    let before = *value;
    if !disabled {
        ui.drag("opacity", value, 0.0..=1.0, 150., false);
        for key in ui.keys("opacity") {
            let step = if key.mods.shift { 0.001 } else { 0.01 };
            *value = match key.key {
                Key::Right | Key::Up => *value + step,
                Key::Left | Key::Down => *value - step,
                Key::Home => 0.,
                Key::End => 1.,
                _ => *value,
            }
            .clamp(0., 1.);
        }
    }
    let control = row([label_text(format!("{:.0}%", *value * 100.))])
        .width(55.)
        .height(22.)
        .align(Align::Center)
        .pad_xy(6., 0.)
        .fill(grey(31))
        .radius(2.)
        .id("opacity")
        .role(Kind::Slider {
            value: *value,
            min: 0.,
            max: 1.,
        })
        .label("Layer opacity")
        .focusable()
        .disabled(disabled)
        .on(State::Focus, |style| style.border(color(ACCENT), 1.))
        .tip("Opacity · drag horizontally or use arrow keys");
    (control, *value != before)
}

/// Owns the host gesture across MUI's delayed keyboard Begin/End delivery.
/// A keyboard value is read by the response tree before its atomic edges arrive;
/// that later Begin must adopt the existing preview rather than cancel it.
#[derive(Default)]
pub(super) struct OpacityDispatch {
    active_for: Option<NodeId>,
}
impl OpacityDispatch {
    pub(super) fn reset(&mut self) {
        self.active_for = None;
    }
    fn apply(
        &mut self,
        ui: &Ui,
        editor: &mut Editor,
        value: f64,
        changed: bool,
        blocked: bool,
    ) -> AppResult<()> {
        let selected = editor.selected_node()?;
        if self.active_for != selected || !editor.gesture_active() {
            self.reset();
        }
        if blocked {
            return Ok(());
        }
        let begin = ui.edits_for("opacity").any(|edge| edge == mui::Edit::Begin);
        let end = ui.edits_for("opacity").any(|edge| edge == mui::Edit::End);
        if (begin || changed) && self.active_for.is_none() && selected.is_some() {
            editor.begin_opacity()?;
            self.active_for = selected;
        }
        if changed && self.active_for.is_some() {
            editor.opacity(value)?;
        }
        if end && self.active_for.is_some() {
            editor.commit()?;
            self.reset();
        }
        Ok(())
    }
}
pub(super) fn color_wheel(fill: [f64; 4], hue: f64) -> El {
    let (_, saturation, value) = rgb_to_hsv(fill);
    canvas(move |size| {
        let center = Point::new(size.width / 2., size.height / 2.);
        let radius = size.height.min(size.width) / 2. - 3.;
        let at = |angle: f64, radius: f64| {
            Point::new(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            )
        };
        let mut draws = Vec::with_capacity(224);
        for i in 0..72 {
            let start = f64::from(i) * std::f64::consts::TAU / 72.;
            let end = f64::from(i + 1) * std::f64::consts::TAU / 72.;
            let rgb = hsv_to_rgb(f64::from(i) * 5., 1., 1., 1.);
            draws.push(Draw::fill(
                Path::polyline(
                    [
                        at(start, radius),
                        at(end, radius),
                        at(end, radius - 12.),
                        at(start, radius - 12.),
                    ],
                    true,
                ),
                mui::scene::Color::srgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32),
            ));
        }
        let [a, b, c] = triangle(center, radius - 18., hue);
        let mix = |u: f64, v: f64| {
            Point::new(
                a.x * u + b.x * v + c.x * (1. - u - v),
                a.y * u + b.y * v + c.y * (1. - u - v),
            )
        };
        let pure = hsv_to_rgb(hue, 1., 1., 1.);
        for u in 0..12 {
            for v in 0..12 - u {
                let u0 = f64::from(u) / 12.;
                let v0 = f64::from(v) / 12.;
                let rgb = |uh: f64, vw: f64| {
                    mui::scene::Color::srgb(
                        (pure[0] * uh + vw) as f32,
                        (pure[1] * uh + vw) as f32,
                        (pure[2] * uh + vw) as f32,
                    )
                };
                draws.push(Draw::fill(
                    Path::polyline(
                        [mix(u0, v0), mix(u0 + 1. / 12., v0), mix(u0, v0 + 1. / 12.)],
                        true,
                    ),
                    rgb(u0 + 1. / 36., v0 + 1. / 36.),
                ));
                if u + v < 11 {
                    draws.push(Draw::fill(
                        Path::polyline(
                            [
                                mix(u0 + 1. / 12., v0),
                                mix(u0 + 1. / 12., v0 + 1. / 12.),
                                mix(u0, v0 + 1. / 12.),
                            ],
                            true,
                        ),
                        rgb(u0 + 2. / 36., v0 + 2. / 36.),
                    ));
                }
            }
        }
        for point in [
            at(hue.to_radians(), radius - 6.),
            mix(saturation * value, (1. - saturation) * value),
        ] {
            let ring: Vec<_> = (0..24)
                .map(|i| {
                    let angle = f64::from(i) * std::f64::consts::TAU / 24.;
                    Point::new(point.x + angle.cos() * 3., point.y + angle.sin() * 3.)
                })
                .collect();
            draws.push(Draw::stroke(Path::polyline(ring, true), grey(242), 1.5));
        }
        draws
    })
    .no_fill()
}
#[derive(Clone, Copy)]
pub(super) enum WheelPart {
    Hue,
    Tone,
}
pub(super) fn wheel_part(point: Point, size: Size, hue: f64) -> Option<WheelPart> {
    let center = Point::new(size.width / 2., size.height / 2.);
    let radius = size.height.min(size.width) / 2. - 3.;
    let distance = (point.x - center.x).hypot(point.y - center.y);
    if distance <= radius && distance >= radius - 12. {
        return Some(WheelPart::Hue);
    }
    let [u, v, w] = weights(point, triangle(center, radius - 18., hue));
    (u >= 0. && v >= 0. && w >= 0.).then_some(WheelPart::Tone)
}
pub(super) fn wheel_pick(
    point: Point,
    size: Size,
    part: WheelPart,
    current: [f64; 4],
    hue: f64,
) -> ([f64; 4], f64) {
    let center = Point::new(size.width / 2., size.height / 2.);
    let radius = size.height.min(size.width) / 2. - 3.;
    let (_, saturation, value) = rgb_to_hsv(current);
    match part {
        WheelPart::Hue => {
            let next = (point.y - center.y)
                .atan2(point.x - center.x)
                .to_degrees()
                .rem_euclid(360.);
            (hsv_to_rgb(next, saturation, value, current[3]), next)
        }
        WheelPart::Tone => {
            let [u, v, _] = weights(point, triangle(center, radius - 18., hue));
            let mut u = u.clamp(0., 1.);
            let mut v = v.clamp(0., 1.);
            let sum = u + v;
            if sum > 1. {
                u /= sum;
                v /= sum;
            }
            let value = u + v;
            (
                hsv_to_rgb(
                    hue,
                    if value == 0. { 0. } else { u / value },
                    value,
                    current[3],
                ),
                hue,
            )
        }
    }
}
fn weights(point: Point, [a, b, c]: [Point; 3]) -> [f64; 3] {
    let divisor = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    let u = ((b.y - c.y) * (point.x - c.x) + (c.x - b.x) * (point.y - c.y)) / divisor;
    let v = ((c.y - a.y) * (point.x - c.x) + (a.x - c.x) * (point.y - c.y)) / divisor;
    [u, v, 1. - u - v]
}
pub(super) fn rgb_to_hsv(rgb: [f64; 4]) -> (f64, f64, f64) {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    let delta = max - min;
    let hue = if delta == 0. {
        0.
    } else if max == rgb[0] {
        60. * ((rgb[1] - rgb[2]) / delta).rem_euclid(6.)
    } else if max == rgb[1] {
        60. * ((rgb[2] - rgb[0]) / delta + 2.)
    } else {
        60. * ((rgb[0] - rgb[1]) / delta + 4.)
    };
    (hue, if max == 0. { 0. } else { delta / max }, max)
}
pub(super) fn hsv_to_rgb(hue: f64, saturation: f64, value: f64, alpha: f64) -> [f64; 4] {
    let h = hue.rem_euclid(360.) / 60.;
    let c = value * saturation;
    let x = c * (1. - (h.rem_euclid(2.) - 1.).abs());
    let m = value - c;
    let rgb = match h as u8 {
        0 => [c, x, 0.],
        1 => [x, c, 0.],
        2 => [0., c, x],
        3 => [0., x, c],
        4 => [x, 0., c],
        _ => [c, 0., x],
    };
    [rgb[0] + m, rgb[1] + m, rgb[2] + m, alpha]
}
fn triangle(center: Point, radius: f64, hue: f64) -> [Point; 3] {
    [0., 120., 240.].map(|offset| {
        let angle = (hue + offset).to_radians();
        Point::new(
            center.x + angle.cos() * radius,
            center.y + angle.sin() * radius,
        )
    })
}
pub(super) fn rulers(artwork: El, size: Size, center: DocumentPoint, zoom: f64) -> El {
    let width = (size.width - TRAY - STUDIO - RULER - 1.).max(1.);
    let height = (size.height - 33. - 53. - 31. - 28. - 26. - RULER).max(1.);
    let ruler = |horizontal: bool, length: f64, middle: f64| {
        let step = ruler_step(zoom);
        let first = ((middle - length / (2. * zoom)) / step).floor() as i64;
        let count = (length / (step * zoom)).ceil().min(200.) as i64 + 2;
        let marks: Vec<_> = (first..first + count)
            .map(|i| {
                (
                    i as f64 * step,
                    (i as f64 * step - middle) * zoom + length / 2.,
                )
            })
            .collect();
        let lines = marks.clone();
        let mut items = vec![
            canvas(move |_| {
                lines
                    .iter()
                    .flat_map(|(_, position)| {
                        (0..5).map(move |minor| {
                            let p = position + f64::from(minor) * step * zoom / 5.;
                            let tick = if minor == 0 { 8. } else { 4. };
                            let points = if horizontal {
                                [Point::new(p, RULER - tick), Point::new(p, RULER)]
                            } else {
                                [Point::new(RULER - tick, p), Point::new(RULER, p)]
                            };
                            Draw::stroke(Path::polyline(points, false), grey(204), 1.)
                        })
                    })
                    .collect()
            })
            .grow(1.)
            .no_fill(),
        ];
        for (value, position) in marks {
            if position < 0. || position >= length {
                continue;
            }
            items.push(
                muted(format!("{value:.0}"))
                    .text_size(8.)
                    .anchor(Align::Start, Align::Start)
                    .offset(
                        if horizontal { position + 2. } else { 1. },
                        if horizontal { 0. } else { position + 1. },
                    )
                    .float(),
            );
        }
        let result = overlay(items).fill(grey(69)).clip();
        if horizontal {
            result.height(RULER).grow(1.)
        } else {
            result.width(RULER).height(pct(100.))
        }
    };
    column([
        row([
            leaf(RULER, RULER).fill(grey(69)),
            ruler(true, width, center.x),
        ])
        .height(RULER),
        row([ruler(false, height, center.y), artwork])
            .grow(1.)
            .no_fill(),
    ])
    .grow(1.)
    .no_fill()
}
fn ruler_step(zoom: f64) -> f64 {
    let desired = 70. / zoom;
    let power = 10_f64.powf(desired.log10().floor());
    [1., 2., 5., 10.]
        .into_iter()
        .map(|factor| factor * power)
        .find(|step| *step >= desired)
        .unwrap_or(power * 10.)
}

impl App {
    fn button(&mut self, id: &str, title: &str, disabled: bool, command: Option<Command>) -> El {
        let blocked = disabled || self.blocked();
        queue_command(
            &self.ui,
            &mut self.commands,
            id,
            blocked || !self.chrome_input,
            command,
        );
        text_button(id, title, blocked)
    }
    fn tool_button(&mut self, tool: Tool, id: &str, title: &str) -> El {
        let activated = activated(&self.ui, id);
        let blocked = self.blocked();
        if activated && !blocked && self.chrome_input && !self.editor.gesture_active() {
            self.editor.tool = tool;
        }
        let selected = self.editor.tool == tool;
        let icon = match tool {
            Tool::Move => chrome::Icon::Move,
            Tool::Rectangle => chrome::Icon::Rectangle,
            Tool::Ellipse => chrome::Icon::Ellipse,
            Tool::Crop => chrome::Icon::Crop,
            Tool::Pan => chrome::Icon::Hand,
        };
        chrome::icon_button(id, title, icon, blocked, selected, 36.)
    }
    fn icon_command(
        &mut self,
        id: &str,
        title: &str,
        icon: chrome::Icon,
        disabled: bool,
        command: Command,
        size: f64,
    ) -> El {
        let blocked = disabled || self.blocked();
        queue_command(
            &self.ui,
            &mut self.commands,
            id,
            blocked || !self.chrome_input,
            Some(command),
        );
        chrome::icon_button(id, title, icon, blocked, false, size)
            .fill(chrome::grey(59))
            .radius(3.)
    }
    pub(super) fn tree(&mut self, size: Size) -> AppResult<El> {
        let blocked = self.blocked();
        let mut menus = vec![brand()];
        for (id, name, available) in [
            ("menu-file", "File", true),
            ("menu-edit", "Edit", true),
            ("menu-document", "Document", true),
            ("menu-text", "Text", false),
            ("menu-vector", "Vector", false),
            ("menu-pixel", "Pixel", false),
            ("menu-layer", "Layer", true),
            ("menu-view", "View", true),
            ("menu-window", "Window", false),
            ("menu-help", "Help", false),
        ] {
            let response = self.ui.get(id);
            if !blocked
                && self.chrome_input
                && available
                && (response.clicked_with(Button::Primary)
                    || self
                        .ui
                        .keys(id)
                        .iter()
                        .any(|key| matches!(key.key, Key::Enter | Key::Space)))
            {
                self.menu = if self.menu == Some(name) {
                    None
                } else {
                    Some(name)
                };
            }
            menus.push(
                self.button(id, name, !available, None)
                    .pad_xy(4., 0.)
                    .no_fill()
                    .tip(if available {
                        name.to_owned()
                    } else {
                        format!("{name} commands are not implemented")
                    }),
            );
        }
        menus.push(spacer());
        menus.push(
            muted(short_label(&self.editor.title, 42))
                .width((size.width - 520.).max(0.))
                .clip(),
        );
        let menu_bar = row(menus)
            .height(33.)
            .align(Align::Center)
            .gap(2.)
            .pad_xy(6., 0.)
            .fill(chrome::grey(40))
            .clip();
        let history = self.editor.session.history_status();
        let (undo_disabled, redo_disabled) = (history.undo == 0, history.redo == 0);
        let mut toolbar = vec![self.icon_command(
            "open",
            "Open document…",
            chrome::Icon::Home,
            self.file_busy,
            Command::Open,
            36.,
        )];
        let mut personas = Vec::new();
        for (id, title, persona) in [
            ("persona-vector", "Vector", chrome::Persona::Vector),
            ("persona-pixel", "Pixel", chrome::Persona::Pixel),
            ("persona-layout", "Layout", chrome::Persona::Layout),
        ] {
            if !blocked
                && self.chrome_input
                && !self.editor.gesture_active()
                && activated(&self.ui, id)
            {
                self.chrome.persona = persona;
                match persona {
                    chrome::Persona::Vector => {
                        self.editor.tool = Tool::Move;
                        self.chrome.lower_tab = chrome::LowerTab::Transform;
                    }
                    chrome::Persona::Pixel => {
                        self.editor.tool = Tool::Crop;
                        self.chrome.layer_tab = chrome::LayerTab::Effects;
                    }
                    chrome::Persona::Layout => {
                        self.editor.tool = Tool::Move;
                        self.chrome.lower_tab = chrome::LowerTab::Pages;
                    }
                }
            }
            let selected = self.chrome.persona == persona;
            personas.push(
                row([label_text(title).text_size(11.)])
                    .id(id)
                    .role(Kind::Button)
                    .label(format!("{title} workspace"))
                    .focusable()
                    .disabled(blocked)
                    .height(31.)
                    .pad_xy(12., 0.)
                    .align(Align::Center)
                    .radius(6.)
                    .fill(chrome::grey(if selected { 30 } else { 48 }))
                    .border(chrome::grey(if selected { 102 } else { 48 }), 1.)
                    .on(State::Focus, |style| style.border(color(ACCENT), 1.)),
            );
        }
        personas.push(
            self.button("persona-slice", "Slice", true, None)
                .height(31.)
                .tip("Slice workspace is not implemented"),
        );
        personas.push(
            self.button("persona-ai", "AI", true, None)
                .height(31.)
                .tip("AI services are not implemented"),
        );
        toolbar.push(
            row(personas)
                .gap(2.)
                .pad(2.)
                .radius(8.)
                .fill(chrome::grey(48)),
        );
        toolbar.extend([
            chrome::separator(),
            self.icon_command(
                "fit",
                "Fit document · Ctrl+0",
                chrome::Icon::Fit,
                false,
                Command::Fit,
                28.,
            ),
            self.icon_command(
                "actual",
                "Actual size · Ctrl+1",
                chrome::Icon::Actual,
                false,
                Command::Actual,
                28.,
            ),
        ]);
        // The same actions remain available in File/Edit and their shortcuts.
        // Narrow native windows do not retain off-screen focusable toolbar nodes.
        if size.width >= 800. {
            toolbar.extend([
                chrome::separator(),
                self.icon_command(
                    "save",
                    "Save · Ctrl+S",
                    chrome::Icon::Save,
                    self.file_busy,
                    Command::Save,
                    28.,
                ),
                self.icon_command(
                    "import",
                    "Place PNG…",
                    chrome::Icon::Open,
                    self.file_busy,
                    Command::Import,
                    28.,
                ),
                self.icon_command(
                    "export",
                    "Export PNG…",
                    chrome::Icon::Export,
                    self.file_busy,
                    Command::Export,
                    28.,
                ),
                chrome::separator(),
                self.icon_command(
                    "undo",
                    "Undo · Ctrl+Z",
                    chrome::Icon::Undo,
                    undo_disabled,
                    Command::Undo,
                    28.,
                ),
                self.icon_command(
                    "redo",
                    "Redo · Ctrl+Shift+Z",
                    chrome::Icon::Redo,
                    redo_disabled,
                    Command::Redo,
                    28.,
                ),
            ]);
        }
        toolbar.push(spacer());
        let command_bar = row(toolbar)
            .height(53.)
            .pad_xy(5., 6.)
            .align(Align::Center)
            .gap(2.)
            .fill(Gradient::vertical(chrome::grey(59), chrome::grey(51)))
            .clip();
        let dirty = if self.editor.dirty() || self.name_edited {
            " •"
        } else {
            ""
        };
        let snapshot = self.editor.snapshot();
        let active = self.editor.page()?;
        let page = snapshot.page(active).ok_or("The active page is missing.")?;
        let page_index = snapshot
            .page_order()
            .iter()
            .position(|page| *page == active)
            .ok_or("The active page order is missing.")?;
        let page_count = snapshot.page_order().len();
        let shown_zoom = self.chrome_plan.map_or(self.view.zoom, DisplayPlan::zoom);
        let tab = row([
            row([label_text(format!(
                "{}{dirty} ({:.1}%)",
                short_label(&self.editor.title, 42),
                shown_zoom * 100.
            ))
            .clip()])
            .pad_xy(8., 0.)
            .height(28.)
            .width(285.)
            .align(Align::Center)
            .fill(chrome::grey(54))
            .border(chrome::grey(31), 1.)
            .label("Active document"),
            spacer(),
        ])
        .height(28.)
        .gap(0.)
        .pad_xy(0., 0.)
        .align(Align::Center)
        .fill(color(STRIP));
        let tool_name = match self.editor.tool {
            Tool::Move => "Move Tool",
            Tool::Rectangle => "Rectangle Tool",
            Tool::Ellipse => "Ellipse Tool",
            Tool::Crop => "Crop Tool",
            Tool::Pan => "Hand Tool",
        };
        let mut context = vec![muted(tool_name).width(94.), muted("Fill")];
        for (index, rgba) in SWATCHES.into_iter().enumerate() {
            let id = format!("fill-{index}");
            if !blocked && self.chrome_input && activated(&self.ui, &id) {
                self.editor.set_fill(rgba)?;
            }
            context.push(
                leaf(18., 18.)
                    .fill(mui::scene::Color::srgb(
                        rgba[0] as f32,
                        rgba[1] as f32,
                        rgba[2] as f32,
                    ))
                    .border(color([0.54, 0.56, 0.60]), 1.)
                    .radius(2.)
                    .id(id)
                    .role(Kind::Button)
                    .label(format!("Set fill color {}", index + 1))
                    .focusable()
                    .on(State::Focus, |style| style.border(color(ACCENT), 2.))
                    .disabled(blocked),
            );
        }
        context.extend([
            chrome::separator(),
            self.button("context-stroke", "Stroke: None", true, None)
                .tip("Stroke editing is not implemented"),
            spacer(),
            muted(format!(
                "{} × {} px · sRGB",
                page.size().width(),
                page.size().height()
            )),
        ]);
        let context_bar = row(context)
            .height(30.)
            .align(Align::Center)
            .gap(4.)
            .pad_xy(5., 2.)
            .fill(chrome::grey(59))
            .clip();
        let rail = column([
            self.tool_button(Tool::Move, "tool-move", "Move Tool (V)"),
            chrome::unavailable("tool-node", "Node Tool", chrome::Icon::Node, 36.),
            chrome::unavailable("tool-pen", "Pen Tool", chrome::Icon::Pen, 36.),
            chrome::unavailable("tool-brush", "Vector Brush Tool", chrome::Icon::Brush, 36.),
            self.tool_button(Tool::Rectangle, "tool-rect", "Rectangle Tool (R)"),
            self.tool_button(Tool::Ellipse, "tool-ellipse", "Ellipse Tool (E)"),
            chrome::unavailable("tool-text", "Artistic Text Tool", chrome::Icon::Text, 36.),
            self.tool_button(Tool::Crop, "tool-crop", "Crop Tool (C)"),
            chrome::unavailable(
                "tool-artboard",
                "Artboard Tool",
                chrome::Icon::Artboard,
                36.,
            ),
            chrome::divider(),
            self.tool_button(Tool::Pan, "tool-pan", "Hand Tool (H)"),
            self.icon_command(
                "tool-zoom",
                "Zoom In",
                chrome::Icon::Zoom,
                false,
                Command::ZoomIn,
                36.,
            ),
            spacer(),
            chrome::unavailable("tool-customize", "Customize Tools", chrome::Icon::More, 36.),
        ])
        .width(chrome::TRAY)
        .pad_xy(2., 2.)
        .gap(0.)
        .clip()
        .fill(chrome::grey(64));
        let selection = if self.chrome_plan.is_some_and(|plan| {
            plan.evaluation.key.revision == self.editor.snapshot().revision()
                || (self.canvas_session.is_some() && self.editor.gesture_active())
        }) {
            match self.editor.selected_node()? {
                Some(node) => selection_bounds(&self.editor.snapshot(), node)?,
                None => None,
            }
        } else {
            None
        };
        let mapping = self
            .chrome_plan
            .zip(self.viewport)
            .map(|(plan, frame)| plan.logical_affine(DocumentPoint::new(frame.x, frame.y)));
        let canvas = canvas(move |_| {
            let (Some(bounds), Some(mapping)) = (selection, mapping) else {
                return Vec::new();
            };
            let rect = mapping.transform_rect_bbox(bounds);
            selection_draw(rect)
        })
        .grow(1.)
        .no_fill()
        .clip()
        .id("viewport")
        .label("Artwork canvas")
        .cursor(if self.editor.tool == Tool::Pan {
            Cursor::Grab
        } else {
            Cursor::Crosshair
        })
        .disabled(blocked);
        let inspector = self.inspector(blocked, page_index, page_count, size)?;
        let document = column([
            tab,
            chrome::rulers(
                canvas,
                size,
                self.chrome_plan
                    .zip(self.viewport)
                    .map_or(self.view.center, |(plan, frame)| plan.center(frame)),
                shown_zoom,
            ),
        ])
        .grow(1.)
        .no_fill();
        let body = row([
            rail,
            document,
            leaf(1., 1.).height(pct(100.)).fill(chrome::grey(13)),
            inspector,
        ])
        .grow(1.)
        .no_fill();
        let rendering = self.desired.is_some_and(|desired| {
            self.host
                .as_ref()
                .and_then(Host::accepted_key)
                .is_none_or(|key| !key.matches(Some(desired)))
        });
        let mut status = self.status.clone();
        if self.file_busy {
            status = "Working on file…".into();
        } else if self.host.is_none() {
            status = "Preparing native graphics…".into();
        } else if self
            .chrome_plan
            .is_some_and(|plan| plan.evaluation.key.revision != self.editor.snapshot().revision())
        {
            status = "Updating artwork…".into();
        } else if self.chrome_plan.is_some_and(|plan| plan.pending)
            || self.navigation.quiet_at.is_some()
        {
            status = "Updating view…".into();
        } else if rendering {
            status = "Updating artwork…".into();
        }
        let status_bar = row([
            muted(status).grow(1.).clip(),
            spacer(),
            self.button("zoom-out", "−", false, Some(Command::ZoomOut)),
            label_text(format!("{:.0}%", shown_zoom * 100.)).width(46.),
            self.button("zoom-in", "+", false, Some(Command::ZoomIn)),
        ])
        .height(25.)
        .align(Align::Center)
        .pad_xy(10., 0.)
        .gap(6.)
        .fill(chrome::grey(51));
        let mut main = vec![menu_bar, command_bar, context_bar, chrome::divider()];
        if let Some(error) = &self.error {
            let message = error.clone();
            let clicked = activated(&self.ui, "dismiss-error");
            let dismiss = text_button("dismiss-error", "Dismiss", blocked);
            if clicked && !blocked && self.chrome_input {
                self.error = None;
            }
            main.push(
                row([
                    text(message)
                        .fill(color([1., 0.80, 0.75]))
                        .text_size(12.)
                        .lines(3)
                        .grow(1.),
                    dismiss,
                ])
                .pad_xy(12., 7.)
                .gap(10.)
                .fill(color([0.29, 0.15, 0.15])),
            );
        }
        main.extend([body, chrome::divider(), status_bar]);
        let mut layers = vec![column(main).size(size.width, size.height).no_fill()];
        if let Some(menu) = self.menu.filter(|_| !blocked) {
            let id = match menu {
                "File" => "menu-file",
                "Edit" => "menu-edit",
                "Document" => "menu-document",
                "Layer" => "menu-layer",
                _ => "menu-view",
            };
            let trigger = self
                .ui
                .scene()
                .and_then(|scene| scene.surface(id))
                .map(|surface| surface.frame)
                .ok_or("The menu trigger has no resolved geometry.")?;
            layers.push(
                self.menu_tree(menu)?
                    .anchor(Align::Start, Align::Start)
                    .offset(trigger.x, trigger.y + trigger.size.height)
                    .float(),
            );
        }
        if self.after_save.is_some() {
            layers.push(if self.uncertain_save.is_some() {
                self.durability_modal(size)
            } else {
                self.close_modal(size)
            });
        }
        if self.dialog_open {
            layers.push(
                overlay([panel([
                    label_text("Choose a file in the native dialog"),
                    muted("Cancel the dialog to return to the artwork."),
                ])
                .width(340.)
                .anchor(Align::Center, Align::Center)])
                .size(size.width, size.height)
                .fill(mui::scene::Color::srgba(0., 0., 0., 0.40))
                .id("native-dialog-shield")
                .float(),
            );
        }
        Ok(overlay(layers).size(size.width, size.height).no_fill())
    }
    fn inspector(
        &mut self,
        blocked: bool,
        page_index: usize,
        page_count: usize,
        size: Size,
    ) -> AppResult<El> {
        let snapshot = self.editor.snapshot();
        let page = self.editor.page()?;
        let roots = snapshot.page(page).ok_or("The page is missing.")?.roots();
        let mut stack: Vec<(NodeId, usize)> = roots.iter().map(|id| (*id, 0)).collect();
        let mut ordered = Vec::with_capacity(snapshot.layers().count());
        while let Some((id, depth)) = stack.pop() {
            let layer = snapshot.node(id).ok_or("The layer is missing.")?;
            ordered.push((id, depth));
            stack.extend(layer.children().iter().map(|child| (*child, depth + 1)));
        }
        // Only visible rows are shaped and painted; spacers retain exact scroll extent.
        let layer_list_id = format!("layer-list-{}", page.get());
        let start = (self.ui.scroll(&layer_list_id)[1].max(0.) / chrome::ROW).floor() as usize;
        let start = start.saturating_sub(2).min(ordered.len());
        let visible_height = self
            .ui
            .scene()
            .and_then(|scene| scene.surface(&layer_list_id))
            .map_or((size.height - 500.).max(64.), |surface| {
                surface.frame.size.height
            });
        let end = (start + (visible_height / chrome::ROW).ceil() as usize + 4).min(ordered.len());
        let mut rows = Vec::new();
        if start > 0 {
            rows.push(leaf(1., start as f64 * chrome::ROW));
        }
        for &(id, depth) in &ordered[start..end] {
            let layer = snapshot.node(id).ok_or("The layer is missing.")?;
            let key = format!("layer-{}", id.get());
            if !blocked && self.chrome_input && activated(&self.ui, &key) {
                self.commit_name()?;
                self.editor.select(id)?;
            }
            let visible_id = format!("visible-{}", id.get());
            let clicked = activated(&self.ui, &visible_id);
            if !blocked && self.chrome_input && clicked {
                self.editor.toggle_visible(id)?;
            }
            let swatch = match layer.content() {
                Content::Vector(vector) => {
                    let [r, g, b, _] = vector.fill().components();
                    let encode = |value: f64| {
                        if value <= 0.0031308 {
                            value * 12.92
                        } else {
                            1.055 * value.powf(1. / 2.4) - 0.055
                        }
                    };
                    mui::scene::Color::srgb(encode(r) as f32, encode(g) as f32, encode(b) as f32)
                }
                Content::Raster { .. } => color([0.22, 0.40, 0.51]),
                Content::Group => color([0.50, 0.52, 0.56]),
            };
            rows.push(
                row([
                    leaf(28., 28.).fill(swatch).border(chrome::grey(31), 1.),
                    label_text(layer.name()).grow(1.).clip(),
                    chrome::icon_button(
                        &visible_id,
                        if layer.visible() {
                            "Hide layer"
                        } else {
                            "Show layer"
                        },
                        chrome::Icon::Eye,
                        blocked,
                        false,
                        20.,
                    )
                    .no_fill()
                    .opacity(if layer.visible() { 1. } else { 0.35 }),
                ])
                .gap(6.)
                .pad_xy(6. + depth.min(8) as f64 * 8., 1.)
                .height(chrome::ROW)
                .align(Align::Center)
                .fill(color(if self.editor.selected == Some(id) {
                    [102. / 255.; 3]
                } else {
                    [51. / 255.; 3]
                }))
                .border(chrome::grey(31), 1.)
                .id(key)
                .role(Kind::Button)
                .label(format!("Select layer {}", layer.name()))
                .focusable()
                .on(State::Focus, |style| style.border(color(ACCENT), 1.5))
                .disabled(blocked)
                .tip(layer.name()),
            );
        }
        if end < ordered.len() {
            rows.push(leaf(1., (ordered.len() - end) as f64 * chrome::ROW));
        }
        let selected = self.editor.selected.is_some();
        if !blocked && self.chrome_input {
            if activated(&self.ui, "studio-layers") {
                self.chrome.layer_tab = chrome::LayerTab::Layers;
            }
            if activated(&self.ui, "studio-effects") {
                self.chrome.layer_tab = chrome::LayerTab::Effects;
            }
        }
        let header = row([
            chrome::tab(
                "studio-layers",
                "Layers",
                self.chrome.layer_tab == chrome::LayerTab::Layers,
                blocked,
            ),
            chrome::tab("studio-line", "Line Style", false, true)
                .tip("Stroke styling is not implemented"),
            chrome::tab(
                "studio-effects",
                "Effects",
                self.chrome.layer_tab == chrome::LayerTab::Effects,
                blocked,
            ),
            chrome::tab("studio-styles", "Styles", false, true)
                .tip("Reusable styles are not implemented"),
        ])
        .height(28.)
        .align(Align::Center)
        .gap(1.)
        .pad_xy(2., 2.)
        .fill(chrome::grey(31));
        let layer_list = column(rows)
            .gap(0.)
            .grow(1.)
            .scroll()
            .clip()
            .id(layer_list_id);
        let mut properties = Vec::new();
        let mut selection_strip = row([
            muted("Opacity:"),
            chrome::opacity(&self.ui, &mut 1., true).0,
            self.button("blend-mode", "Normal", true, None).grow(1.),
            chrome::unavailable("blend-options", "Blend Options", chrome::Icon::More, 20.),
            chrome::unavailable("layer-lock", "Lock Layer", chrome::Icon::Lock, 20.),
        ])
        .height(34.)
        .pad_xy(6., 6.)
        .gap(4.)
        .align(Align::Center)
        .fill(chrome::grey(61));
        let mut rename = leaf(1., 1.);
        if let Some(node) = self.editor.selected_node()? {
            let layer = snapshot
                .node(node)
                .ok_or("The selected layer is missing.")?;
            if self.name_node != Some(node) {
                self.name_node = Some(node);
                self.name = layer.name().into();
                self.name_edited = false;
            }
            let previous = self.name.clone();
            let (name, edited) = text_input(&mut self.ui, "layer-name", &mut self.name);
            if self.name.len() > 4096 {
                self.name = previous;
                return Err("Layer names are limited to 4096 bytes. Enter a shorter name.".into());
            }
            self.name_edited |= edited;
            let commit_name = self
                .ui
                .keys("layer-name")
                .iter()
                .any(|key| key.key == Key::Enter)
                || !self.ui.focused("layer-name");
            if self.name_edited
                && commit_name
                && !blocked
                && self.chrome_input
                && !self.editor.gesture_active()
            {
                let mut spec = layer_spec(layer);
                spec.name = self.name.clone();
                self.editor
                    .transact("Rename layer", [Change::SetLayer { node, layer: spec }])?;
                self.name_edited = false;
            }
            rename = name
                .width(pct(100.))
                .height(22.)
                .fill(chrome::grey(31))
                .radius(2.)
                .disabled(blocked)
                .label("Layer name");
            let mut opacity = layer.opacity();
            let (control, changed) =
                chrome::opacity(&self.ui, &mut opacity, blocked || !self.chrome_input);
            if self.chrome_input {
                self.chrome.opacity_dispatch.apply(
                    &self.ui,
                    &mut self.editor,
                    opacity,
                    changed,
                    blocked,
                )?;
            }
            selection_strip = row([
                muted("Opacity:"),
                control.disabled(blocked),
                self.button("blend-mode", "Normal", true, None)
                    .grow(1.)
                    .tip("Blend mode editing is not implemented"),
                chrome::unavailable("blend-options", "Blend Options", chrome::Icon::More, 20.),
                chrome::unavailable("layer-lock", "Lock Layer", chrome::Icon::Lock, 20.),
            ])
            .height(34.)
            .pad_xy(6., 6.)
            .gap(4.)
            .align(Align::Center)
            .fill(chrome::grey(61));
            let exposure = layer
                .operations()
                .iter()
                .find_map(|op| match op {
                    LiveOperation::Exposure(stops) => Some(*stops),
                    _ => None,
                })
                .unwrap_or(0.);
            let less = activated(&self.ui, "exposure-less");
            let more = activated(&self.ui, "exposure-more");
            let minus = self.button("exposure-less", "−", blocked, None);
            let plus = self.button("exposure-more", "+", blocked, None);
            if !blocked && self.chrome_input && !self.editor.gesture_active() {
                if less {
                    self.editor.exposure(-0.25)?;
                }
                if more {
                    self.editor.exposure(0.25)?;
                }
            }
            properties.push(
                row([
                    muted("Exposure"),
                    spacer(),
                    minus,
                    label_text(format!("{exposure:+.2} EV")).width(66.),
                    plus,
                ])
                .align(Align::Center)
                .gap(6.),
            );
            let crop = layer.operations().iter().find_map(|op| match op {
                LiveOperation::Crop(rect) => Some(*rect),
                _ => None,
            });
            let clear = activated(&self.ui, "clear-crop");
            let crop_button = self.button("clear-crop", "Clear", blocked || crop.is_none(), None);
            if !blocked && self.chrome_input && clear && crop.is_some() {
                self.editor.clear_crop()?;
            }
            properties.push(
                row([
                    muted(if crop.is_some() {
                        "Crop · editable"
                    } else {
                        "Crop · none"
                    }),
                    spacer(),
                    crop_button,
                ])
                .align(Align::Center),
            );
            let toggle = activated(&self.ui, "mask");
            let mask = self.button(
                "mask",
                if layer.mask().is_some() {
                    "Disable mask"
                } else {
                    "Add ellipse mask"
                },
                blocked || matches!(layer.content(), Content::Group),
                None,
            );
            if !blocked && self.chrome_input && toggle {
                self.editor.toggle_mask()?;
            }
            properties.push(mask.width(pct(100.)));
            properties.push(muted(match layer.content() {
                Content::Raster { .. } => "Original pixels preserved",
                Content::Vector(_) => "Editable Bézier geometry",
                Content::Group => "Isolated group composition",
            }));
            properties.push(self.button(
                REMOVE_PROPERTIES,
                "Remove layer",
                false,
                Some(Command::Delete),
            ));
        } else {
            self.name_node = None;
            properties.extend([
                muted("Select artwork or a layer."),
                muted("Draw a rectangle or ellipse to begin."),
            ]);
        }
        let mask_disabled = self
            .editor
            .selected_node()?
            .and_then(|node| snapshot.node(node))
            .is_none_or(|layer| matches!(layer.content(), Content::Group));
        if !blocked && self.chrome_input && !mask_disabled && activated(&self.ui, "layer-mask") {
            self.editor.toggle_mask()?;
        }
        if !blocked && self.chrome_input && selected && activated(&self.ui, "layer-fx") {
            self.chrome.layer_tab = chrome::LayerTab::Effects;
        }
        let footer = row([
            self.icon_command(
                "lower",
                "Lower layer",
                chrome::Icon::Down,
                !selected,
                Command::Lower,
                20.,
            ),
            self.icon_command(
                "raise",
                "Raise layer",
                chrome::Icon::Up,
                !selected,
                Command::Raise,
                20.,
            ),
            chrome::icon_button(
                "layer-fx",
                "Editable layer operations",
                chrome::Icon::Effects,
                blocked || !selected,
                false,
                20.,
            ),
            chrome::icon_button(
                "layer-mask",
                "Toggle editable ellipse mask",
                chrome::Icon::Mask,
                blocked || mask_disabled,
                false,
                20.,
            ),
            chrome::unavailable("layer-filter", "Live Filter", chrome::Icon::Adjust, 20.),
            chrome::unavailable("layer-group", "Group Layers", chrome::Icon::Group, 20.),
            chrome::unavailable("layer-pixel", "New Pixel Layer", chrome::Icon::Pixel, 20.),
            spacer(),
            self.icon_command(
                REMOVE_FOOTER,
                "Delete layer",
                chrome::Icon::Delete,
                !selected,
                Command::Delete,
                20.,
            ),
        ])
        .height(26.)
        .pad_xy(4., 2.)
        .align(Align::Center)
        .gap(2.)
        .fill(chrome::grey(72));
        let content = if self.chrome.layer_tab == chrome::LayerTab::Layers {
            layer_list
        } else {
            column(properties)
                .gap(6.)
                .pad(8.)
                .grow(1.)
                .scroll()
                .clip()
                .id(format!("properties-{}", page.get()))
        };
        let layer_studio = column([
            header,
            selection_strip,
            chrome::divider(),
            content,
            row([
                muted("Name:").width(35.).shrink(0.),
                rename.width(0.).grow(1.),
            ])
            .gap(6.)
            .pad_xy(6., 3.)
            .height(28.)
            .align(Align::Center),
            footer,
        ])
        .grow(1.)
        .fill(chrome::grey(51))
        .clip();
        let color_studio = self.color_studio(blocked)?;
        let lower_studio = self.lower_studio(blocked, page_index, page_count)?;
        Ok(column([
            color_studio,
            chrome::divider(),
            layer_studio,
            chrome::divider(),
            lower_studio,
        ])
        .width(chrome::STUDIO)
        .fill(chrome::grey(61)))
    }
    fn color_studio(&mut self, blocked: bool) -> AppResult<El> {
        if !blocked && self.chrome_input {
            if activated(&self.ui, "studio-color") {
                self.chrome.color_tab = chrome::ColorTab::Color;
            }
            if activated(&self.ui, "studio-swatches") {
                self.chrome.color_tab = chrome::ColorTab::Swatches;
            }
        }
        let header = row([
            chrome::tab(
                "studio-color",
                "Color",
                self.chrome.color_tab == chrome::ColorTab::Color,
                blocked,
            ),
            chrome::tab(
                "studio-swatches",
                "Swatches",
                self.chrome.color_tab == chrome::ColorTab::Swatches,
                blocked,
            ),
            chrome::tab("studio-stroke", "Stroke", false, true)
                .tip("Stroke editing is not implemented"),
            chrome::tab("studio-appearance", "Appearance", false, true)
                .tip("Appearance editing is not implemented"),
        ])
        .height(28.)
        .align(Align::Center)
        .gap(1.)
        .pad_xy(2., 2.)
        .fill(chrome::grey(31));
        let mut swatches = Vec::new();
        for (index, rgba) in SWATCHES.into_iter().enumerate() {
            let id = format!("studio-fill-{index}");
            if !blocked && self.chrome_input && activated(&self.ui, &id) {
                self.editor.set_fill(rgba)?;
            }
            swatches.push(
                leaf(24., 24.)
                    .fill(mui::scene::Color::srgb(
                        rgba[0] as f32,
                        rgba[1] as f32,
                        rgba[2] as f32,
                    ))
                    .id(id)
                    .role(Kind::Button)
                    .label(format!("Set fill color {}", index + 1))
                    .focusable()
                    .disabled(blocked)
                    .border(chrome::grey(31), 1.)
                    .on(State::Focus, |style| style.border(color(ACCENT), 2.)),
            );
        }
        let mut fill = self.current_fill()?;
        let (hue, saturation, _) = chrome::rgb_to_hsv(fill);
        if self.chrome.wheel_drag.is_none() && saturation > 0. {
            self.chrome.color_hue = hue;
        }
        let response = self.ui.get("fill-wheel");
        let wheel_frame = self
            .ui
            .scene()
            .and_then(|scene| scene.surface("fill-wheel"))
            .map(|surface| surface.frame);
        if !blocked
            && self.chrome_input
            && let (Some(frame), Some(point)) = (wheel_frame, self.ui.local("fill-wheel"))
        {
            if response.pressed
                && let Some(part) = chrome::wheel_part(point, frame.size, self.chrome.color_hue)
            {
                self.editor.begin_fill()?;
                self.chrome.wheel_drag = Some(part);
            }
            if response.held
                && (response.pressed || response.drag_delta != Point::ZERO)
                && let Some(part) = self.chrome.wheel_drag
            {
                let (next, hue) =
                    chrome::wheel_pick(point, frame.size, part, fill, self.chrome.color_hue);
                self.editor.fill(next)?;
                self.chrome.color_hue = hue;
                fill = next;
            }
            if response.released && self.chrome.wheel_drag.take().is_some() {
                self.editor.commit()?;
            }
        }
        if !blocked && self.chrome_input && self.chrome.wheel_drag.is_none() {
            let mut next_hue = self.chrome.color_hue;
            for key in self.ui.keys("fill-wheel") {
                match key.key {
                    Key::Right | Key::Up => next_hue += 1.,
                    Key::Left | Key::Down => next_hue -= 1.,
                    _ => {}
                }
            }
            if next_hue != self.chrome.color_hue {
                let (_, saturation, value) = chrome::rgb_to_hsv(fill);
                fill = chrome::hsv_to_rgb(next_hue, saturation, value, fill[3]);
                self.editor.begin_fill()?;
                self.editor.fill(fill)?;
                self.editor.commit()?;
                self.chrome.color_hue = next_hue.rem_euclid(360.);
            }
        }
        let body = if self.chrome.color_tab == chrome::ColorTab::Swatches {
            column([muted("Document palette"), row(swatches).gap(4.), spacer()])
                .pad(8.)
                .gap(8.)
                .grow(1.)
        } else {
            column([
                row([
                    leaf(26., 26.)
                        .fill(mui::scene::Color::srgb(
                            fill[0] as f32,
                            fill[1] as f32,
                            fill[2] as f32,
                        ))
                        .border(chrome::grey(31), 1.),
                    spacer(),
                    self.button("color-model", "RGB", true, None)
                        .tip("RGB fill readout"),
                ])
                .align(Align::Center),
                chrome::color_wheel(fill, self.chrome.color_hue)
                    .height(136.)
                    .width(pct(100.))
                    .id("fill-wheel")
                    .role(Kind::Slider {
                        value: self.chrome.color_hue,
                        min: 0.,
                        max: 360.,
                    })
                    .label("Fill color wheel")
                    .focusable()
                    .disabled(blocked)
                    .on(State::Focus, |style| style.border(color(ACCENT), 1.))
                    .tip("Fill color · drag hue ring or triangle; arrow keys change hue"),
                row([
                    chrome::readout("R", format!("{:.0}", fill[0] * 255.), 74.),
                    chrome::readout("G", format!("{:.0}", fill[1] * 255.), 74.),
                    chrome::readout("B", format!("{:.0}", fill[2] * 255.), 74.),
                ])
                .gap(3.),
                chrome::readout(
                    "Hex",
                    format!(
                        "#{:02X}{:02X}{:02X}",
                        (fill[0] * 255.).round() as u8,
                        (fill[1] * 255.).round() as u8,
                        (fill[2] * 255.).round() as u8
                    ),
                    200.,
                ),
            ])
            .pad(6.)
            .gap(5.)
            .grow(1.)
        };
        Ok(column([header, body])
            .height(248.)
            .fill(chrome::grey(61))
            .clip())
    }
    fn current_fill(&self) -> AppResult<[f64; 4]> {
        let snapshot = self.editor.snapshot();
        let fill = self
            .editor
            .selected_node()?
            .and_then(|node| snapshot.node(node))
            .and_then(|layer| match layer.content() {
                Content::Vector(vector) => Some(vector.fill().components()),
                _ => None,
            })
            .unwrap_or(self.editor.fill);
        let encode = |value: f64| {
            if value <= 0.0031308 {
                value * 12.92
            } else {
                1.055 * value.powf(1. / 2.4) - 0.055
            }
        };
        Ok([encode(fill[0]), encode(fill[1]), encode(fill[2]), fill[3]])
    }
    fn lower_studio(
        &mut self,
        blocked: bool,
        page_index: usize,
        page_count: usize,
    ) -> AppResult<El> {
        if !blocked && self.chrome_input {
            if activated(&self.ui, "studio-transform") {
                self.chrome.lower_tab = chrome::LowerTab::Transform;
            }
            if activated(&self.ui, "studio-history") {
                self.chrome.lower_tab = chrome::LowerTab::History;
            }
            if activated(&self.ui, "studio-pages") {
                self.chrome.lower_tab = chrome::LowerTab::Pages;
            }
        }
        let header = row([
            chrome::tab(
                "studio-transform",
                "Transform",
                self.chrome.lower_tab == chrome::LowerTab::Transform,
                blocked,
            ),
            chrome::tab(
                "studio-history",
                "History",
                self.chrome.lower_tab == chrome::LowerTab::History,
                blocked,
            ),
            chrome::tab(
                "studio-pages",
                "Pages",
                self.chrome.lower_tab == chrome::LowerTab::Pages,
                blocked,
            ),
        ])
        .height(28.)
        .align(Align::Center)
        .gap(1.)
        .pad_xy(2., 2.)
        .fill(chrome::grey(31));
        let body = match self.chrome.lower_tab {
            chrome::LowerTab::Transform => {
                let bounds = self
                    .editor
                    .selected_node()?
                    .map(|node| selection_bounds(&self.editor.snapshot(), node))
                    .transpose()?
                    .flatten();
                let values = bounds.map(|b| [b.x0, b.y0, b.width(), b.height()]);
                let value = |i: usize| {
                    values.map_or_else(|| "—".into(), |values| format!("{:.1} px", values[i]))
                };
                column([
                    row([
                        chrome::readout("X:", value(0), 119.),
                        chrome::readout("Y:", value(1), 119.),
                    ])
                    .gap(6.),
                    row([
                        chrome::readout("W:", value(2), 119.),
                        chrome::readout("H:", value(3), 119.),
                    ])
                    .gap(6.),
                ])
                .gap(5.)
                .pad(6.)
                .grow(1.)
                .label("Selection transform readout")
            }
            chrome::LowerTab::History => {
                let history = self.editor.session.history_status();
                let (undo, redo) = (history.undo, history.redo);
                column([
                    row([
                        self.button("history-undo", "Undo", undo == 0, Some(Command::Undo)),
                        self.button("history-redo", "Redo", redo == 0, Some(Command::Redo)),
                    ])
                    .gap(6.),
                    muted(format!("{undo} undo · {redo} redo")),
                ])
                .pad(6.)
                .gap(5.)
                .grow(1.)
            }
            chrome::LowerTab::Pages => column([
                row([
                    self.button(
                        "previous-page",
                        "‹",
                        page_index == 0,
                        Some(Command::PreviousPage),
                    )
                    .label("Previous page"),
                    label_text(format!("{} / {page_count}", page_index + 1)).grow(1.),
                    self.button(
                        "next-page",
                        "›",
                        page_index + 1 == page_count,
                        Some(Command::NextPage),
                    )
                    .label("Next page"),
                ])
                .gap(6.)
                .align(Align::Center),
                self.button(
                    "add-page",
                    "Add page",
                    page_count >= document_limits().max_pages,
                    Some(Command::AddPage),
                ),
            ])
            .pad(6.)
            .gap(5.)
            .grow(1.),
        };
        Ok(column([header, body])
            .height(105.)
            .fill(chrome::grey(61))
            .clip())
    }
    fn menu_tree(&mut self, menu: &str) -> AppResult<El> {
        let entries: &[(Command, &str)] = match menu {
            "File" => &[
                (Command::New, "New…"),
                (Command::Open, "Open…"),
                (Command::Import, "Place PNG…"),
                (Command::Save, "Save"),
                (Command::SaveAs, "Save As…"),
                (Command::Export, "Export PNG…"),
                (Command::Quit, "Quit"),
            ],
            "Edit" => &[
                (Command::Undo, "Undo"),
                (Command::Redo, "Redo"),
                (Command::Delete, "Delete"),
            ],
            "Document" => &[
                (Command::AddPage, "Add page"),
                (Command::PreviousPage, "Previous page"),
                (Command::NextPage, "Next page"),
            ],
            "Layer" => &[
                (Command::Raise, "Raise layer"),
                (Command::Lower, "Lower layer"),
                (Command::Delete, "Delete"),
            ],
            _ => &[
                (Command::Fit, "Fit artwork"),
                (Command::Actual, "Actual size · 100%"),
                (Command::ZoomIn, "Zoom in"),
                (Command::ZoomOut, "Zoom out"),
                (Command::PreviousPage, "Previous page"),
                (Command::NextPage, "Next page"),
            ],
        };
        let mut items = Vec::new();
        for (index, (command, title)) in entries.iter().enumerate() {
            let history = self.editor.session.history_status();
            let disabled = match command {
                Command::Undo => history.undo == 0,
                Command::Redo => history.redo == 0,
                Command::Delete | Command::Raise | Command::Lower => self.editor.selected.is_none(),
                Command::New
                | Command::Open
                | Command::Save
                | Command::SaveAs
                | Command::Import
                | Command::Export
                | Command::Quit => self.file_busy,
                Command::PreviousPage => self.editor.adjacent_page(false)?.is_none(),
                Command::NextPage => self.editor.adjacent_page(true)?.is_none(),
                Command::AddPage => {
                    self.editor.snapshot().page_order().len() >= document_limits().max_pages
                }
                _ => false,
            };
            let shortcut = match command {
                Command::New => "Ctrl+N",
                Command::Open => "Ctrl+O",
                Command::Save => "Ctrl+S",
                Command::SaveAs => "Ctrl+Shift+S",
                Command::Quit => "Ctrl+Q",
                Command::Undo => "Ctrl+Z",
                Command::Redo => "Ctrl+Shift+Z",
                Command::Fit => "Ctrl+0",
                Command::Actual => "Ctrl+1",
                Command::Delete => "Del",
                Command::PreviousPage => "Ctrl+Page Up",
                Command::NextPage => "Ctrl+Page Down",
                _ => "",
            };
            let id = format!("popup-{index}");
            if !disabled && !self.blocked() && activated(&self.ui, &id) {
                self.commands.push(*command);
            }
            items.push(
                row([
                    leaf(20., 16.),
                    label_text(*title).grow(1.),
                    muted(shortcut).width(87.),
                ])
                .height(25.)
                .pad_xy(5., 0.)
                .align(Align::Center)
                .gap(3.)
                .id(id)
                .role(Kind::Button)
                .label(*title)
                .focusable()
                .disabled(disabled || self.blocked())
                .no_fill()
                .on(State::Hover, |style| style.fill(chrome::grey(70)))
                .on(State::Focus, |style| style.border(color(ACCENT), 1.)),
            );
        }
        Ok(column(items)
            .width(280.)
            .pad(3.)
            .gap(0.)
            .fill(mui::scene::Color::srgb(28. / 255., 27. / 255., 27. / 255.))
            .border(chrome::grey(82), 1.)
            .radius(2.))
    }
    fn close_modal(&mut self, size: Size) -> El {
        let save_clicked = self.chrome_input && activated(&self.ui, "modal-save");
        let discard_clicked = self.chrome_input && activated(&self.ui, "modal-discard");
        let cancel_clicked = self.chrome_input && activated(&self.ui, "modal-cancel");
        let save = text_button("modal-save", "Save", self.file_busy);
        let discard = text_button("modal-discard", "Discard", self.file_busy);
        let cancel = text_button("modal-cancel", "Cancel", false);
        if cancel_clicked {
            self.after_save = None;
            self.uncertain_save = None;
            self.ui.cancel();
        }
        if discard_clicked && !self.file_busy {
            self.finish_after_save();
        }
        if save_clicked && !self.file_busy {
            self.commands.push(Command::Save);
        }
        let modal = panel([
            label_text(format!("Save changes to {}?", self.editor.title)).text_size(14.),
            muted("Your original pixels and editable layers will be saved."),
            row([cancel, spacer(), discard, save])
                .gap(8.)
                .align(Align::Center),
        ])
        .width(420.)
        .gap(18.)
        .pad(20.)
        .radius(4.)
        .anchor(Align::Center, Align::Center);
        overlay([modal])
            .size(size.width, size.height)
            .fill(mui::scene::Color::srgba(0., 0., 0., 0.58))
            .id("modal-shield")
            .float()
    }
    fn durability_modal(&mut self, size: Size) -> El {
        let keep_clicked = self.chrome_input && activated(&self.ui, "durability-keep");
        let proceed_clicked = self.chrome_input && activated(&self.ui, "durability-proceed");
        let keep = text_button("durability-keep", "Keep open", false);
        let proceed = text_button(
            "durability-proceed",
            if matches!(self.after_save, Some(AfterSave::Close)) {
                "Close anyway"
            } else {
                "Continue anyway"
            },
            false,
        );
        let message = self.uncertain_save.clone().unwrap_or_default();
        if keep_clicked {
            self.after_save = None;
            self.uncertain_save = None;
            self.ui.cancel();
        }
        if proceed_clicked {
            self.finish_after_save();
        }
        let modal = panel([
            label_text("The save needs attention").text_size(14.),
            label_text(message).width(pct(100.)).lines(5),
            muted("The file was published. Keep it open to retry the save."),
            row([spacer(), keep, proceed]).gap(8.).align(Align::Center),
        ])
        .width(440.)
        .gap(18.)
        .pad(20.)
        .radius(4.)
        .anchor(Align::Center, Align::Center);
        overlay([modal])
            .size(size.width, size.height)
            .fill(mui::scene::Color::srgba(0., 0., 0., 0.58))
            .id("durability-shield")
            .float()
    }
}

fn rectangle(rect: Rect) -> mui::geometry::Path {
    mui::geometry::Path::polyline(
        [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x1, rect.y1),
            Point::new(rect.x0, rect.y1),
        ],
        true,
    )
}
fn selection_draw(rect: Rect) -> Vec<Draw> {
    let mut draws = vec![Draw::stroke(rectangle(rect), color(ACCENT), 1.)];
    for (x, y) in [
        (rect.x0, rect.y0),
        (rect.x1, rect.y0),
        (rect.x0, rect.y1),
        (rect.x1, rect.y1),
    ] {
        draws.push(Draw::fill(
            rectangle(Rect::new(x - 3., y - 3., x + 3., y + 3.)),
            color([0.92, 0.96, 1.]),
        ));
    }
    draws
}
fn brand() -> El {
    let mark = canvas(|_| {
        vec![
            Draw::stroke(
                Path::polyline(
                    [
                        Point::new(3., 18.),
                        Point::new(10., 3.),
                        Point::new(17., 18.),
                    ],
                    false,
                ),
                color(ACCENT),
                2.,
            ),
            Draw::stroke(
                Path::polyline([Point::new(7., 12.), Point::new(13., 12.)], false),
                color(ACCENT),
                2.,
            ),
        ]
    })
    .size(20., 21.);
    row([mark])
        .align(Align::Center)
        .gap(5.)
        .width(24.)
        .label("AFFINT")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui() -> AppResult<Ui> {
        Ok(Ui::new(Theme::DEFAULT).font(Font::new(
            &include_bytes!("../../assets/fonts/AdwaitaSans-Regular.ttf")[..],
        )?))
    }
    struct OpacityHost {
        ui: Ui,
        editor: Editor,
        dispatch: OpacityDispatch,
    }
    impl OpacityHost {
        fn new() -> AppResult<Self> {
            let editor = Editor::demo()?;
            let mut host = Self {
                ui: ui()?,
                editor,
                dispatch: OpacityDispatch::default(),
            };
            let revision = host.editor.snapshot().revision();
            host.editor.saved(revision, "opacity.affint".into());
            host.paint(PointerInput::default())?;
            Ok(host)
        }
        fn value(&self) -> AppResult<f64> {
            let node = self
                .editor
                .selected_node()?
                .ok_or("Missing opacity selection.")?;
            Ok(self
                .editor
                .snapshot()
                .node(node)
                .ok_or("Missing opacity layer.")?
                .opacity())
        }
        fn author(&mut self) -> AppResult<El> {
            let mut value = self.value()?;
            let (control, changed) = opacity(&self.ui, &mut value, false);
            self.dispatch
                .apply(&self.ui, &mut self.editor, value, changed, false)?;
            Ok(control)
        }
        fn frame(&mut self, input: Input) -> AppResult<()> {
            let root = self.author()?;
            self.ui.frame(root, Some(Size::new(55., 22.)), input, 0.)?;
            Ok(())
        }
        fn input(&mut self, input: Input) -> AppResult<()> {
            let pointer = input.pointer;
            self.frame(input)?;
            // Same response-authoring replay as App::refresh for real input.
            self.frame(Input::from(pointer))
        }
        fn paint(&mut self, pointer: PointerInput) -> AppResult<()> {
            // Same one-tree path as App::refresh for a graphics/animation wake.
            self.frame(Input::from(pointer))
        }
        fn cancel(&mut self) -> AppResult<()> {
            self.ui.cancel();
            self.dispatch.reset();
            self.editor.cancel()?;
            self.paint(PointerInput::default())?;
            self.paint(PointerInput::default())
        }
    }
    fn key(key: Key) -> Input {
        Input {
            keys: vec![KeyPress {
                key,
                mods: Mods::default(),
            }],
            ..Default::default()
        }
    }
    fn pointer(x: f64, held: bool) -> PointerInput {
        PointerInput {
            pos: Some(Point::new(x, 11.)),
            buttons: Buttons::default().set(Button::Primary, held),
            ..Default::default()
        }
    }

    #[test]
    fn keyboard_opacity_adopts_delayed_atomic_edges_and_commits_one_edit() -> AppResult<()> {
        let mut host = OpacityHost::new()?;
        host.ui.focus("opacity");
        host.input(key(Key::Left))?;
        assert!((host.value()? - 0.99).abs() < 1e-12);
        assert!(host.editor.gesture_active());
        assert_eq!(host.editor.session.history_status().undo, 0);
        assert_eq!(
            host.ui.edits_for("opacity").collect::<Vec<_>>(),
            [mui::Edit::Begin, mui::Edit::End]
        );
        host.paint(PointerInput::default())?;
        assert!((host.value()? - 0.99).abs() < 1e-12);
        assert!(!host.editor.gesture_active());
        assert_eq!(host.editor.session.history_status().undo, 1);
        host.editor.history(History::Undo)?;
        assert_eq!(host.value()?, 1.);
        host.editor.history(History::Redo)?;
        assert!((host.value()? - 0.99).abs() < 1e-12);
        for (input, expected, undo) in [(Key::Home, 0., 2), (Key::End, 1., 3)] {
            host.input(key(input))?;
            host.paint(PointerInput::default())?;
            assert_eq!(host.value()?, expected);
            assert_eq!(host.editor.session.history_status().undo, undo);
            assert!(!host.editor.gesture_active());
        }
        Ok(())
    }

    #[test]
    fn pointer_opacity_hold_commit_and_host_cancellation_preserve_history() -> AppResult<()> {
        let mut host = OpacityHost::new()?;
        host.input(Input::from(pointer(30., true)))?;
        host.input(Input::from(pointer(0., true)))?;
        assert!((host.value()? - 0.8).abs() < 1e-12);
        let held_revision = host.editor.snapshot().revision();
        host.paint(pointer(0., true))?;
        host.paint(pointer(0., true))?;
        assert_eq!(host.editor.snapshot().revision(), held_revision);
        assert_eq!(host.editor.session.history_status().undo, 0);
        host.input(Input::from(pointer(-15., true)))?;
        assert!((host.value()? - 0.7).abs() < 1e-12);
        host.input(Input::from(pointer(-15., false)))?;
        host.paint(pointer(-15., false))?;
        assert!(!host.editor.gesture_active());
        assert_eq!(host.editor.session.history_status().undo, 1);
        host.editor.history(History::Undo)?;
        assert_eq!(host.value()?, 1.);
        host.editor.history(History::Redo)?;
        assert!((host.value()? - 0.7).abs() < 1e-12);

        let mut canceled = OpacityHost::new()?;
        let original = canceled.editor.snapshot().revision();
        canceled.input(Input::from(pointer(30., true)))?;
        canceled.input(Input::from(pointer(0., true)))?;
        canceled.cancel()?;
        assert_eq!(canceled.value()?, 1.);
        assert_eq!(canceled.editor.snapshot().revision(), original);
        assert_eq!(canceled.editor.session.history_status().undo, 0);
        assert!(!canceled.editor.dirty());
        assert!(!canceled.editor.gesture_active());
        Ok(())
    }

    #[test]
    fn rulers_reserve_twenty_units_in_each_axis_at_reference_and_minimum_sizes() -> AppResult<()> {
        for window in [
            Size::new(500., 640.),
            Size::new(1024., 720.),
            Size::new(1440., 940.),
        ] {
            let document = Size::new(
                window.width - TRAY - STUDIO - 1.,
                window.height - 33. - 53. - 31. - 28. - 26.,
            );
            let root = rulers(
                // A zero-height leaf remains zero in the row's cross axis and
                // MUI omits collapsed surfaces. Production uses an auto-sized
                // canvas, which stretches vertically inside the artwork row.
                canvas(|_| Vec::new())
                    .grow(1.)
                    .no_fill()
                    .clip()
                    .id("viewport")
                    .label("Artwork canvas")
                    .cursor(Cursor::Crosshair),
                window,
                DocumentPoint::new(480., 360.),
                1.,
            )
            .size(document.width, document.height);
            let mut ui = ui()?;
            ui.frame(root, Some(document), Input::default(), 0.)?;
            let frame = ui
                .scene()
                .and_then(|scene| scene.surface("viewport"))
                .ok_or("Missing ruler artwork surface.")?
                .frame;
            assert_eq!(frame.x, RULER);
            assert_eq!(frame.y, RULER);
            assert_eq!(frame.size.width, document.width - RULER);
            assert_eq!(frame.size.height, document.height - RULER);
        }
        Ok(())
    }

    #[test]
    fn distinct_properties_and_footer_delete_controls_route_one_enabled_activation() -> AppResult<()>
    {
        let mut ui = ui()?;
        let mut commands = Vec::new();
        let author = |ui: &Ui, commands: &mut Vec<Command>, disabled| {
            queue_command(
                ui,
                commands,
                REMOVE_PROPERTIES,
                disabled,
                Some(Command::Delete),
            );
            queue_command(ui, commands, REMOVE_FOOTER, disabled, Some(Command::Delete));
            row([
                text_button(REMOVE_PROPERTIES, "Remove layer", disabled),
                icon_button(
                    REMOVE_FOOTER,
                    "Delete layer",
                    Icon::Delete,
                    disabled,
                    false,
                    20.,
                ),
            ])
        };
        let root = author(&ui, &mut commands, false);
        ui.frame(root, None, Input::default(), 0.)?;
        let scene = ui.scene().ok_or("Missing delete control scene.")?;
        assert_ne!(
            scene
                .surface(REMOVE_PROPERTIES)
                .ok_or("Missing properties delete.")?
                .frame
                .x,
            scene
                .surface(REMOVE_FOOTER)
                .ok_or("Missing footer delete.")?
                .frame
                .x
        );
        for id in [REMOVE_PROPERTIES, REMOVE_FOOTER] {
            ui.focus(id);
            let root = author(&ui, &mut commands, false);
            ui.frame(root, None, key(Key::Enter), 0.)?;
            let root = author(&ui, &mut commands, false);
            assert_eq!(commands.len(), 1);
            assert!(matches!(commands[0], Command::Delete));
            commands.clear();
            ui.frame(root, None, Input::default(), 0.)?;
        }
        ui.focus(REMOVE_FOOTER);
        let root = author(&ui, &mut commands, true);
        ui.frame(root, None, key(Key::Enter), 0.)?;
        let root = author(&ui, &mut commands, true);
        assert!(commands.is_empty());
        ui.frame(root, None, Input::default(), 0.)?;
        Ok(())
    }
}
