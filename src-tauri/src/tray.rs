use crate::core::format::fmt_aic;
use crate::core::pace::{Pace, Status};
use crate::core::settings::Settings;

/// Copilot glyph from @primer/octicons 19.42.0 (MIT, GitHub Inc.).
const COPILOT_GLYPH: [&str; 2] = [
    "M7.998 15.035c-4.562 0-7.873-2.914-7.998-3.749V9.338c.085-.628.677-1.686 1.588-2.065.013-.07.024-.143.036-.218.029-.183.06-.384.126-.612-.201-.508-.254-1.084-.254-1.656 0-.87.128-1.769.693-2.484.579-.733 1.494-1.124 2.724-1.261 1.206-.134 2.262.034 2.944.765.05.053.096.108.139.165.044-.057.094-.112.143-.165.682-.731 1.738-.899 2.944-.765 1.23.137 2.145.528 2.724 1.261.566.715.693 1.614.693 2.484 0 .572-.053 1.148-.254 1.656.066.228.098.429.126.612.012.076.024.148.037.218.924.385 1.522 1.471 1.591 2.095v1.872c0 .766-3.351 3.795-8.002 3.795Zm0-1.485c2.28 0 4.584-1.11 5.002-1.433V7.862l-.023-.116c-.49.21-1.075.291-1.727.291-1.146 0-2.059-.327-2.71-.991A3.222 3.222 0 0 1 8 6.303a3.24 3.24 0 0 1-.544.743c-.65.664-1.563.991-2.71.991-.652 0-1.236-.081-1.727-.291l-.023.116v4.255c.419.323 2.722 1.433 5.002 1.433ZM6.762 2.83c-.193-.206-.637-.413-1.682-.297-1.019.113-1.479.404-1.713.7-.247.312-.369.789-.369 1.554 0 .793.129 1.171.308 1.371.162.181.519.379 1.442.379.853 0 1.339-.235 1.638-.54.315-.322.527-.827.617-1.553.117-.935-.037-1.395-.241-1.614Zm4.155-.297c-1.044-.116-1.488.091-1.681.297-.204.219-.359.679-.242 1.614.091.726.303 1.231.618 1.553.299.305.784.54 1.638.54.922 0 1.28-.198 1.442-.379.179-.2.308-.578.308-1.371 0-.765-.123-1.242-.37-1.554-.233-.296-.693-.587-1.713-.7Z",
    "M6.25 9.037a.75.75 0 0 1 .75.75v1.501a.75.75 0 0 1-1.5 0V9.787a.75.75 0 0 1 .75-.75Zm4.25.75v1.501a.75.75 0 0 1-1.5 0V9.787a.75.75 0 0 1 1.5 0Z",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Item { id: &'static str, label: String, accelerator: Option<&'static str>, enabled: bool },
    Separator,
}

pub fn menu(update: &crate::core::updater::UpdateState) -> Vec<Entry> {
    use crate::core::updater::UpdateState;
    let item = |id, label: &str, accelerator, enabled| Entry::Item { id, label: label.into(), accelerator, enabled };
    let mut out = vec![
        item("dashboard", "Open Dashboard…", Some("CmdOrCtrl+D"), true),
        item("refresh", "Refresh Now", Some("CmdOrCtrl+R"), true),
        item("settings", "Settings…", Some("CmdOrCtrl+,"), true),
        Entry::Separator,
    ];
    match update {
        UpdateState::Available { version, .. } => out.push(item("update", &format!("Update to {version}"), None, true)),
        UpdateState::Downloading { version } => out.push(item("update", &format!("Downloading {version}…"), None, false)),
        UpdateState::Installing { version } => out.push(item("update", &format!("Installing {version}…"), None, false)),
        _ => {}
    }
    out.push(match update {
        UpdateState::Checking => item("check-updates", "Checking for Updates…", None, false),
        other => item("check-updates", "Check for Updates…", None, !other.is_busy()),
    });
    out.extend([Entry::Separator, item("quit", "Quit CopilotBar", Some("CmdOrCtrl+Q"), true)]);
    out
}

pub struct Icon {
    /// Straight (not premultiplied) RGBA, as Tauri expects.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct TrayLook {
    /// `None` for number-only: macOS reserves room for any image, so the item must have none.
    pub icon: Option<Icon>,
    pub template: bool,
    pub title: Option<String>,
    pub tooltip: String,
}

type Rgb = [u8; 3];

const BLACK: Rgb = [0, 0, 0];
const WHITE: Rgb = [255, 255, 255];

fn status_colour(status: Status, dark_menu_bar: bool, colorblind: bool) -> Option<Rgb> {
    let (green, orange, red) = match (colorblind, dark_menu_bar) {
        (true, _) => ([0x00, 0x9e, 0x73], [0xe6, 0x9f, 0x00], [0xd5, 0x5e, 0x00]),
        (false, true) => ([0x32, 0xd7, 0x4b], [0xff, 0x9f, 0x0a], [0xff, 0x45, 0x3a]),
        (false, false) => ([0x1f, 0x9d, 0x45], [0xd9, 0x7a, 0x00], [0xe0, 0x35, 0x2b]),
    };
    match status {
        Status::Under | Status::OnPace => Some(green),
        Status::Ahead => Some(orange),
        Status::Exhausted => Some(red),
        Status::NoAllowance => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum IconInk {
    Template,
    Tinted { alert: Rgb, glyph: Rgb },
}

impl IconInk {
    fn tint(self) -> Option<Rgb> {
        match self {
            IconInk::Template => None,
            IconInk::Tinted { alert, .. } => Some(alert),
        }
    }

    fn is_template(self) -> bool {
        matches!(self, IconInk::Template)
    }

    fn colour(self) -> Rgb {
        self.tint().unwrap_or(BLACK)
    }

    fn glyph(self) -> Rgb {
        match self {
            IconInk::Template => BLACK,
            IconInk::Tinted { glyph, .. } => glyph,
        }
    }
}

fn icon_ink(settings: &Settings, pace: &Pace, dark_menu_bar: bool) -> IconInk {
    if !settings.alert_colors || settings.menu_bar_display == "number" {
        return IconInk::Template;
    }
    match status_colour(pace.status, dark_menu_bar, settings.palette == "colorblind") {
        Some(alert) => IconInk::Tinted { alert, glyph: if dark_menu_bar { WHITE } else { BLACK } },
        None => IconInk::Template,
    }
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::NoAllowance => "no Allowance set",
        Status::Under => "Under",
        Status::OnPace => "On Pace",
        Status::Ahead => "Ahead",
        Status::Exhausted => "Exhausted",
    }
}

pub fn look(settings: &Settings, pace: &Pace, dark_menu_bar: bool) -> TrayLook {
    let display = settings.menu_bar_display.as_str();
    let ink = icon_ink(settings, pace, dark_menu_bar);
    let title = (display != "icon").then(|| number_text(&settings.menu_bar_number, pace));
    let used = fmt_aic(pace.used);
    let status = status_label(pace.status);
    let tooltip = match (pace.allowance, pace.frac_used) {
        (Some(b), Some(f)) => format!("Copilot: {used} of {} AIC Allowance used ({:.0}%) · {status}", fmt_aic(b), f * 100.0),
        _ => format!("Copilot: {used} AIC used this Cycle · {status}"),
    };
    if display == "number" {
        return TrayLook { icon: None, template: ink.is_template(), title, tooltip };
    }
    let frac = pace.frac_used.map(|f| f.clamp(0.0, 1.0));
    let mut icon_canvas = Canvas::new();
    match settings.menu_bar_icon.as_str() {
        "disc" => disc(&mut icon_canvas, frac, ink.colour()),
        "ring" => ring(&mut icon_canvas, frac, ink.colour(), ink.glyph()),
        "progress" => progress(&mut icon_canvas, pace, ink.colour(), ink.glyph()),
        _ => copilot(&mut icon_canvas, ink.glyph(), ink.tint()),
    };
    let (rgba, width, height) = icon_canvas.into_rgba();
    TrayLook { icon: Some(Icon { rgba, width, height }), template: ink.is_template(), title, tooltip }
}

fn number_text(number: &str, pace: &Pace) -> String {
    let used = fmt_aic(pace.used);
    match (number, pace.allowance) {
        ("used", _) => used,
        ("left", Some(b)) if pace.used <= b => format!("{} left", fmt_aic(b - pace.used)),
        ("left", Some(b)) => format!("{} over", fmt_aic(pace.used - b)),
        ("pct", Some(_)) => format!("{:.0}%", pace.frac_used.unwrap_or(0.0) * 100.0),
        ("left" | "pct", None) => used,
        (_, Some(b)) => format!("{used} / {}", fmt_aic(b)),
        (_, None) => format!("{used} AIC"),
    }
}

struct Canvas {
    pixmap: tiny_skia::Pixmap,
}

const UNITS: f32 = 16.0;
const ICON_PX: f32 = 36.0;
const SCALE: f32 = ICON_PX / UNITS;

impl Canvas {
    fn new() -> Canvas {
        let side = (UNITS * SCALE).ceil() as u32;
        Canvas { pixmap: tiny_skia::Pixmap::new(side, side).expect("pixmap") }
    }

    fn paint(color: Rgb, opacity: f64, blend_mode: tiny_skia::BlendMode) -> tiny_skia::Paint<'static> {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], (opacity * 255.0).round() as u8);
        paint.anti_alias = true;
        paint.blend_mode = blend_mode;
        paint
    }

    fn base() -> tiny_skia::Transform {
        tiny_skia::Transform::from_scale(SCALE, SCALE)
    }

    fn fill(&mut self, path: &tiny_skia::Path, color: Rgb, opacity: f64, transform: tiny_skia::Transform) {
        let paint = Self::paint(color, opacity, tiny_skia::BlendMode::SourceOver);
        self.pixmap.fill_path(path, &paint, tiny_skia::FillRule::Winding, Self::base().pre_concat(transform), None);
    }

    fn erase(&mut self, path: &tiny_skia::Path, transform: tiny_skia::Transform) {
        let paint = Self::paint(BLACK, 1.0, tiny_skia::BlendMode::DestinationOut);
        self.pixmap.fill_path(path, &paint, tiny_skia::FillRule::Winding, Self::base().pre_concat(transform), None);
    }

    fn stroke(&mut self, path: &tiny_skia::Path, color: Rgb, opacity: f64, stroke: &tiny_skia::Stroke, transform: tiny_skia::Transform) {
        let paint = Self::paint(color, opacity, tiny_skia::BlendMode::SourceOver);
        self.pixmap.stroke_path(path, &paint, stroke, Self::base().pre_concat(transform), None);
    }

    fn into_rgba(self) -> (Vec<u8>, u32, u32) {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let data = self
            .pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        (data, w, h)
    }
}

fn path(d: &str) -> tiny_skia::Path {
    let bez = kurbo::BezPath::from_svg(d).expect("valid tray path");
    let mut b = tiny_skia::PathBuilder::new();
    let f = |v: f64| v as f32;
    for el in bez.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => b.move_to(f(p.x), f(p.y)),
            kurbo::PathEl::LineTo(p) => b.line_to(f(p.x), f(p.y)),
            kurbo::PathEl::QuadTo(p1, p) => b.quad_to(f(p1.x), f(p1.y), f(p.x), f(p.y)),
            kurbo::PathEl::CurveTo(p1, p2, p) => b.cubic_to(f(p1.x), f(p1.y), f(p2.x), f(p2.y), f(p.x), f(p.y)),
            kurbo::PathEl::ClosePath => b.close(),
        }
    }
    b.finish().expect("non-empty tray path")
}

fn circle(cx: f64, cy: f64, r: f64) -> tiny_skia::Path {
    path(&format!(
        "M{} {cy} A{r} {r} 0 0 1 {cx} {} A{r} {r} 0 0 1 {} {cy} A{r} {r} 0 0 1 {cx} {} A{r} {r} 0 0 1 {} {cy} Z",
        cx + r,
        cy + r,
        cx - r,
        cy - r,
        cx + r
    ))
}

fn copilot_glyph() -> &'static [tiny_skia::Path; 2] {
    static GLYPH: once_cell::sync::Lazy<[tiny_skia::Path; 2]> = once_cell::sync::Lazy::new(|| [path(COPILOT_GLYPH[0]), path(COPILOT_GLYPH[1])]);
    &GLYPH
}

fn glyph_transform(s: f64) -> tiny_skia::Transform {
    let t = (8.0 - 8.0 * s) as f32;
    tiny_skia::Transform::from_translate(t, t).pre_scale(s as f32, s as f32)
}

fn arc_stroke(width: f64, cap: tiny_skia::LineCap, r: f64, frac: f64) -> tiny_skia::Stroke {
    let circ = 2.0 * std::f64::consts::PI * r;
    tiny_skia::Stroke { width: width as f32, line_cap: cap, dash: tiny_skia::StrokeDash::new(vec![(circ * frac) as f32, circ as f32], 0.0), ..Default::default() }
}

fn rotate_to_twelve() -> tiny_skia::Transform {
    tiny_skia::Transform::from_rotate_at(-90.0, 8.0, 8.0)
}

fn copilot(c: &mut Canvas, fg: Rgb, tint: Option<Rgb>) {
    let id = tiny_skia::Transform::identity();
    for p in copilot_glyph() {
        c.fill(p, fg, 1.0, id);
    }
    if let Some(dot) = tint {
        c.erase(&circle(12.6, 12.6, 4.1), id);
        c.fill(&circle(12.6, 12.6, 3.1), dot, 1.0, id);
    }
}

fn progress(c: &mut Canvas, pace: &Pace, color: Rgb, fg: Rgb) {
    let (r, sw) = (6.2, 2.4);
    let frac = pace.frac_used.unwrap_or(0.0).clamp(0.0, 1.0);
    let a = 2.0 * std::f64::consts::PI * pace.frac_elapsed - std::f64::consts::FRAC_PI_2;
    let ring = circle(8.0, 8.0, r);
    let track = tiny_skia::Stroke { width: sw as f32, ..Default::default() };
    c.stroke(&ring, fg, 0.3, &track, tiny_skia::Transform::identity());
    c.stroke(&ring, color, 1.0, &arc_stroke(sw, tiny_skia::LineCap::Butt, r, frac), rotate_to_twelve());
    let tick = path(&format!("M{:.2} {:.2} L{:.2} {:.2}", 8.0 + 3.2 * a.cos(), 8.0 + 3.2 * a.sin(), 8.0 + 7.9 * a.cos(), 8.0 + 7.9 * a.sin()));
    let tick_stroke = tiny_skia::Stroke { width: 1.2, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
    c.stroke(&tick, fg, 1.0, &tick_stroke, tiny_skia::Transform::identity());
}

fn disc(c: &mut Canvas, frac: Option<f64>, color: Rgb) {
    let id = tiny_skia::Transform::identity();
    c.fill(&circle(8.0, 8.0, 8.0), color, if frac.is_some() { 0.4 } else { 1.0 }, id);
    match frac {
        Some(f) if f >= 0.999 => c.fill(&circle(8.0, 8.0, 8.0), color, 1.0, id),
        Some(f) if f > 0.004 => {
            let a = 2.0 * std::f64::consts::PI * f - std::f64::consts::FRAC_PI_2;
            let large = if f > 0.5 { 1 } else { 0 };
            c.fill(&path(&format!("M8 8 L8 0 A8 8 0 {large} 1 {:.3} {:.3} Z", 8.0 + 8.0 * a.cos(), 8.0 + 8.0 * a.sin())), color, 1.0, id);
        }
        _ => {}
    }
    for p in copilot_glyph() {
        c.erase(p, glyph_transform(0.75));
    }
}

fn ring(c: &mut Canvas, frac: Option<f64>, color: Rgb, fg: Rgb) {
    let (r, sw) = (7.1, 1.8);
    let ring = circle(8.0, 8.0, r);
    let track = tiny_skia::Stroke { width: sw as f32, ..Default::default() };
    c.stroke(&ring, color, if frac.is_some() { 0.45 } else { 1.0 }, &track, tiny_skia::Transform::identity());
    if let Some(f) = frac.filter(|f| *f > 0.004) {
        c.stroke(&ring, color, 1.0, &arc_stroke(sw, tiny_skia::LineCap::Round, r, f), rotate_to_twelve());
    }
    for p in copilot_glyph() {
        c.fill(p, fg, 1.0, glyph_transform(0.6));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pace::{compute, Cycle};
    use crate::core::updater::UpdateState;

    fn pace(status_used: f64) -> Pace {
        let cycle = Cycle { start: 0, end: 30 * 86_400_000 };
        let mut r = crate::core::model::UsageRecord::new("a".into(), 1000, crate::core::model::Client::CopilotCli, "m");
        r.aic = Some(status_used);
        compute(&[r], cycle, Some(200.0), None, 15 * 86_400_000, 0.05)
    }

    fn mb(display: &str, icon: &str, number: &str) -> Settings {
        Settings { menu_bar_display: display.into(), menu_bar_icon: icon.into(), menu_bar_number: number.into(), ..Settings::default() }
    }

    fn no_allowance_pace() -> Pace {
        let mut p = pace(0.0);
        p.allowance = None;
        p.frac_used = None;
        p.status = Status::NoAllowance;
        p
    }

    fn tray_icon(l: &TrayLook) -> &Icon {
        l.icon.as_ref().expect("look has an icon")
    }

    #[test]
    fn every_icon_renders() {
        for icon in ["disc", "ring", "copilot", "progress"] {
            let l = look(&mb("icon", icon, "pct"), &pace(100.0), false);
            assert_eq!(tray_icon(&l).rgba.len() as u32, tray_icon(&l).width * tray_icon(&l).height * 4, "{icon}");
            assert!(tray_icon(&l).rgba.iter().skip(3).step_by(4).any(|a| *a > 0), "{icon} drew nothing");
            assert_eq!(l.title, None, "icon only has no title");
        }
        let n = look(&mb("icon-number", "disc", "pct"), &pace(100.0), false);
        assert_eq!((tray_icon(&n).width, n.title.as_deref()), (36, Some("50%")));
    }

    #[test]
    fn number_only_has_no_icon_and_a_title() {
        let n = look(&mb("number", "disc", "pct"), &pace(100.0), false);
        assert!(n.icon.is_none(), "number-only draws no icon");
        assert_eq!(n.title.as_deref(), Some("50%"));
        let used = look(&mb("number", "copilot", "used"), &pace(100.0), true);
        assert!(used.icon.is_none(), "number-only ignores the icon style");
        assert_eq!(used.title.as_deref(), Some("100"));
    }

    fn px(l: &TrayLook, x: f64, y: f64) -> [u8; 4] {
        let (px, py) = ((x * 2.25) as u32, (y * 2.25) as u32);
        let i = ((py * tray_icon(l).width + px) * 4) as usize;
        [tray_icon(l).rgba[i], tray_icon(l).rgba[i + 1], tray_icon(l).rgba[i + 2], tray_icon(l).rgba[i + 3]]
    }

    fn opaque(l: &TrayLook) -> impl Iterator<Item = &[u8]> {
        tray_icon(l).rgba.chunks(4).filter(|p| p[3] == 255)
    }

    #[test]
    fn icons_are_36px_at_2x() {
        for icon in ["disc", "ring", "copilot", "progress"] {
            let l = look(&mb("icon", icon, "pct"), &pace(100.0), false);
            assert_eq!((tray_icon(&l).width, tray_icon(&l).height), (36, 36), "{icon}");
        }
    }

    #[test]
    fn copilot_icon_keeps_its_colour_and_adds_a_dot() {
        let l = look(&mb("icon", "copilot", "pct"), &pace(150.0), true);
        assert!(!l.template, "alert dot needs colour");
        assert_eq!(px(&l, 12.6, 12.6), [0xff, 0x9f, 0x0a, 255], "dot centre");
        // The glyph's left eye frame keeps the menu bar colour.
        assert_eq!(px(&l, 1.5, 10.5), [255, 255, 255, 255]);
        let mut no_allowance = pace(0.0);
        no_allowance.allowance = None;
        no_allowance.frac_used = None;
        no_allowance.status = Status::NoAllowance;
        let plain = look(&mb("icon", "copilot", "pct"), &no_allowance, true);
        assert!(plain.template);
        assert!(opaque(&plain).all(|p| p[..3] == [0, 0, 0]), "template glyph is black");
        assert_ne!(px(&plain, 15.15, 10.05)[3], 0, "no cut-out without a status colour");
    }

    #[test]
    fn ring_tints_only_the_ring() {
        let l = look(&mb("icon", "ring", "pct"), &pace(150.0), true);
        assert_eq!(px(&l, 8.0, 0.95), [0xff, 0x9f, 0x0a, 255], "used arc");
        let glyph: Vec<_> = tray_icon(&l).rgba.chunks(4).enumerate().filter(|(i, p)| {
            let (x, y) = ((i % 36) as f64 / 2.25, (i / 36) as f64 / 2.25);
            p[3] > 0 && (x - 8.0).hypot(y - 8.0) < 5.0
        }).map(|(_, p)| p).collect();
        assert!(!glyph.is_empty());
        assert!(glyph.iter().all(|p| p[..3] == [255, 255, 255]), "glyph keeps the menu bar colour");
    }

    #[test]
    fn disc_fills_its_used_share() {
        // Half used: right half solid, left translucent (102 is 40%).
        let l = look(&mb("icon", "disc", "pct"), &pace(100.0), false);
        assert_eq!(px(&l, 14.5, 8.0)[3], 255);
        assert_eq!(px(&l, 1.5, 8.0)[3], 102);
        assert_eq!(px(&l, 8.0, 8.0 - 4.3)[3], 0, "glyph is cut out");
    }

    #[test]
    fn progress_marks_even_pace() {
        // Halfway through the Cycle: the tick points straight down from the centre.
        let l = look(&mb("icon", "progress", "pct"), &pace(100.0), false);
        assert_eq!(px(&l, 8.0, 12.0), [0, 0, 0, 255], "pace tick");
        assert_eq!(px(&l, 14.2, 8.0)[3], 255, "used half of the ring");
        assert_eq!(px(&l, 1.8, 8.0)[3], 77, "unused half is the 30% track");
    }

    #[test]
    fn status_colours_follow_menu_bar_and_palette() {
        let dot = |s: &Settings, used: f64, dark: bool| copilot_dot_colour(&look(s, &pace(used), dark));
        let icon = mb("icon", "copilot", "pct");
        for used in [50.0, 100.0] {
            assert_eq!(dot(&icon, used, true), [0x32, 0xd7, 0x4b], "green on a dark menu bar");
            assert_eq!(dot(&icon, used, false), [0x1f, 0x9d, 0x45], "green on a light menu bar");
        }
        assert_eq!(dot(&icon, 150.0, true), [0xff, 0x9f, 0x0a]);
        assert_eq!(dot(&icon, 150.0, false), [0xd9, 0x7a, 0x00]);
        assert_eq!(dot(&icon, 250.0, true), [0xff, 0x45, 0x3a]);
        assert_eq!(dot(&icon, 250.0, false), [0xe0, 0x35, 0x2b]);
        let cb = Settings { palette: "colorblind".into(), ..icon };
        assert_eq!(dot(&cb, 100.0, false), [0x00, 0x9e, 0x73]);
        assert_eq!(dot(&cb, 100.0, true), [0x00, 0x9e, 0x73]);
        assert_eq!(dot(&cb, 150.0, false), [0xe6, 0x9f, 0x00]);
        assert_eq!(dot(&cb, 250.0, false), [0xd5, 0x5e, 0x00]);
    }

    fn copilot_dot_colour(l: &TrayLook) -> [u8; 3] {
        tray_icon(l).rgba
            .chunks(4)
            .filter(|p| p[3] == 255)
            .max_by_key(|p| p[0].max(p[1]).max(p[2]) as i32 - p[0].min(p[1]).min(p[2]) as i32)
            .map(|p| [p[0], p[1], p[2]])
            .unwrap()
    }

    #[test]
    fn numbers() {
        let t = |n: &str, used: f64| look(&mb("icon-number", "disc", n), &pace(used), false).title.unwrap();
        assert_eq!(t("pct", 100.0), "50%");
        assert_eq!(t("used", 100.0), "100");
        assert_eq!(t("left", 100.0), "100 left");
        assert_eq!(t("used-total", 100.0), "100 / 200");
        assert_eq!(t("left", 250.0), "50.0 over");
    }

    #[test]
    fn default_look_is_the_disc_with_percent_used() {
        let s = Settings::default();
        let on_pace = look(&s, &pace(100.0), false);
        assert_eq!(on_pace.title.as_deref(), Some("50%"));
        assert!(!on_pace.template, "on pace should be tinted green");
        assert!(!look(&s, &pace(50.0), false).template, "under pace should be tinted green");
        let ahead = look(&s, &pace(150.0), true);
        assert!(!ahead.template, "ahead of pace should be tinted");
        assert_eq!(ahead.title.as_deref(), Some("75%"));
        assert!(look(&s, &no_allowance_pace(), true).template, "no Allowance stays monochrome");
        let plain = look(&Settings { alert_colors: false, ..Settings::default() }, &pace(150.0), true);
        assert!(plain.template);
    }

    #[test]
    fn tooltip_uses_glossary_pace_status_names() {
        let s = Settings::default();
        assert_eq!(look(&s, &pace(100.0), false).tooltip, "Copilot: 100 of 200 AIC Allowance used (50%) · On Pace");
        assert_eq!(look(&s, &pace(50.0), false).tooltip, "Copilot: 50.0 of 200 AIC Allowance used (25%) · Under");
        assert_eq!(look(&s, &pace(150.0), false).tooltip, "Copilot: 150 of 200 AIC Allowance used (75%) · Ahead");
        assert_eq!(look(&s, &pace(250.0), false).tooltip, "Copilot: 250 of 200 AIC Allowance used (125%) · Exhausted");
        assert_eq!(look(&s, &no_allowance_pace(), false).tooltip, "Copilot: 0 AIC used this Cycle · no Allowance set");
    }

    #[test]
    fn icon_ink_is_a_template_only_when_it_has_no_alert_colour() {
        let s = mb("icon", "copilot", "pct");
        assert_eq!(icon_ink(&s, &pace(100.0), false), IconInk::Tinted { alert: [0x1f, 0x9d, 0x45], glyph: BLACK });
        assert_eq!(icon_ink(&s, &pace(100.0), true), IconInk::Tinted { alert: [0x32, 0xd7, 0x4b], glyph: WHITE });
        assert_eq!(icon_ink(&s, &no_allowance_pace(), true), IconInk::Template);
        assert_eq!(icon_ink(&Settings { alert_colors: false, ..s.clone() }, &pace(150.0), true), IconInk::Template);
        assert_eq!(icon_ink(&mb("number", "copilot", "pct"), &pace(150.0), true), IconInk::Template);
        assert_eq!(IconInk::Template.colour(), BLACK);
        assert_eq!(IconInk::Template.glyph(), BLACK);
    }

    fn labels(update: &UpdateState) -> Vec<String> {
        menu(update).iter().map(|e| match e {
            Entry::Item { label, enabled: false, .. } => format!("({label})"),
            Entry::Item { label, .. } => label.clone(),
            Entry::Separator => "-".into(),
        }).collect()
    }

    #[test]
    fn menu_offers_check_for_updates_and_the_update_while_one_is_available() {
        let plain = ["Open Dashboard…", "Refresh Now", "Settings…", "-", "Check for Updates…", "-", "Quit CopilotBar"];
        assert_eq!(labels(&UpdateState::Idle), plain);
        assert_eq!(labels(&UpdateState::UpToDate), plain);
        assert_eq!(labels(&UpdateState::Error { message: "x".into(), url: None }), plain);
        assert_eq!(labels(&UpdateState::Checking), ["Open Dashboard…", "Refresh Now", "Settings…", "-", "(Checking for Updates…)", "-", "Quit CopilotBar"]);
        let available = UpdateState::Available { version: "1.3.0".into(), url: "https://github.com/o/n/releases/tag/v1.3.0".into() };
        assert_eq!(labels(&available), ["Open Dashboard…", "Refresh Now", "Settings…", "-", "Update to 1.3.0", "Check for Updates…", "-", "Quit CopilotBar"]);
        let ids: Vec<_> = menu(&available).into_iter().filter_map(|e| match e { Entry::Item { id, .. } => Some(id), _ => None }).collect();
        assert_eq!(ids, ["dashboard", "refresh", "settings", "update", "check-updates", "quit"]);
    }

    #[test]
    fn menu_shows_the_update_downloading_and_installing_and_offers_nothing_else_meanwhile() {
        let downloading = UpdateState::Downloading { version: "1.3.0".into() };
        assert_eq!(labels(&downloading), ["Open Dashboard…", "Refresh Now", "Settings…", "-", "(Downloading 1.3.0…)", "(Check for Updates…)", "-", "Quit CopilotBar"]);
        let installing = UpdateState::Installing { version: "1.3.0".into() };
        assert_eq!(labels(&installing), ["Open Dashboard…", "Refresh Now", "Settings…", "-", "(Installing 1.3.0…)", "(Check for Updates…)", "-", "Quit CopilotBar"]);
    }

    #[test]
    #[ignore]
    fn dump_tray_icons() {
        let dir = std::env::var("TRAY_DUMP").expect("set TRAY_DUMP to an output directory");
        for icon in ["disc", "ring", "copilot", "progress"] {
            for (label, used, alert) in [("none", None, true), ("p10", Some(20.0), true), ("p50", Some(100.0), true), ("ahead", Some(150.0), true), ("over", Some(250.0), true), ("full", Some(200.0), false)] {
                for dark in [false, true] {
                    for cb in [false, true] {
                        let mut s = mb("icon", icon, "pct");
                        s.alert_colors = alert;
                        if cb { s.palette = "colorblind".into(); }
                        let p = match used { Some(u) => pace(u), None => { let mut p = pace(0.0); p.allowance = None; p.frac_used = None; p.status = Status::NoAllowance; p } };
                        let l = look(&s, &p, dark);
                        let name = format!("{dir}/{icon}-{label}-{}-{}-{}x{}.rgba", if dark { "dark" } else { "light" }, if cb { "cb" } else { "std" }, tray_icon(&l).width, tray_icon(&l).height);
                        std::fs::write(name, &tray_icon(&l).rgba).unwrap();
                    }
                }
            }
        }
    }
}
