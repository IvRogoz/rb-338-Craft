use eframe::egui::{
    self, Align2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
};
use std::{f32::consts::PI, sync::Arc};

#[derive(Clone, Copy)]
pub enum Surface {
    Steel,
    Silver,
    Drum,
    Drum909,
    Wood,
}
#[derive(Clone, Copy)]
pub enum KnobKind {
    Dark,
    Synth,
    Red,
    Cream,
    Drum909,
}
pub const INK: Color32 = Color32::from_rgb(0x10, 0x14, 0x11);
pub const LIGHT: Color32 = Color32::from_rgb(0xed, 0xf3, 0xec);
fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0., 1.);
    rgb(
        (a.r() as f32 * (1. - t) + b.r() as f32 * t) as u8,
        (a.g() as f32 * (1. - t) + b.g() as f32 * t) as u8,
        (a.b() as f32 * (1. - t) + b.b() as f32 * t) as u8,
    )
}
pub fn install_fonts(ctx: &Context) {
    let mut defs = FontDefinitions::default();
    let root = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into())
        .join("Fonts");
    if let Ok(data) = std::fs::read(root.join("arial.ttf")) {
        defs.font_data
            .insert("rack-arial".into(), Arc::new(FontData::from_owned(data)));
        defs.families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "rack-arial".into());
    }
    let mut bold = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Ok(data) = std::fs::read(root.join("arialbd.ttf")) {
        defs.font_data.insert(
            "rack-arial-bold".into(),
            Arc::new(FontData::from_owned(data)),
        );
        bold.insert(0, "rack-arial-bold".into());
    }
    defs.families
        .insert(FontFamily::Name("rack-bold".into()), bold);
    ctx.set_fonts(defs);
}
fn gradient(ui: &Ui, r: Rect, stops: &[(f32, Color32)], horizontal: bool) {
    let mut mesh = egui::Mesh::default();
    for pair in stops.windows(2) {
        let (a, ca) = pair[0];
        let (b, cb) = pair[1];
        let (p0, p1, p2, p3) = if horizontal {
            let x0 = r.left() + r.width() * a;
            let x1 = r.left() + r.width() * b;
            (
                egui::pos2(x0, r.top()),
                egui::pos2(x1, r.top()),
                egui::pos2(x1, r.bottom()),
                egui::pos2(x0, r.bottom()),
            )
        } else {
            let y0 = r.top() + r.height() * a;
            let y1 = r.top() + r.height() * b;
            (
                egui::pos2(r.left(), y0),
                egui::pos2(r.right(), y0),
                egui::pos2(r.right(), y1),
                egui::pos2(r.left(), y1),
            )
        };
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(p0, ca);
        mesh.colored_vertex(p1, if horizontal { cb } else { ca });
        mesh.colored_vertex(p2, cb);
        mesh.colored_vertex(p3, if horizontal { ca } else { cb });
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    ui.painter().add(Shape::mesh(mesh));
}
pub fn surface(ui: &Ui, r: Rect, kind: Surface) {
    let (colors, spacing, slant) = match kind {
        Surface::Steel => (
            [
                (0., rgb(131, 141, 144)),
                (0.55, rgb(91, 105, 109)),
                (1., rgb(70, 88, 94)),
            ],
            3.,
            0.45,
        ),
        Surface::Silver => (
            [
                (0., rgb(255, 255, 255)),
                (0.55, rgb(227, 227, 227)),
                (1., rgb(240, 240, 239)),
            ],
            3.,
            0.22,
        ),
        Surface::Drum => (
            [
                (0., rgb(65, 61, 53)),
                (0.65, rgb(55, 53, 47)),
                (1., rgb(68, 65, 56)),
            ],
            3.,
            0.35,
        ),
        Surface::Drum909 => (
            [
                (0., rgb(0xe4, 0xe1, 0xdb)),
                (0.55, rgb(0xc6, 0xc0, 0xba)),
                (1., rgb(0xd8, 0xd2, 0xcc)),
            ],
            3.,
            0.268,
        ),
        Surface::Wood => (
            [
                (0., rgb(34, 44, 45)),
                (0.58, rgb(155, 120, 97)),
                (1., rgb(43, 51, 50)),
            ],
            4.,
            0.03,
        ),
    };
    gradient(ui, r, &colors, true);
    let painter = ui.painter().with_clip_rect(r);
    let mut x = r.left() - r.height() * slant;
    while x < r.right() {
        painter.line_segment(
            [
                egui::pos2(x, r.bottom()),
                egui::pos2(x + r.height() * slant, r.top()),
            ],
            Stroke::new(0.45, Color32::from_white_alpha(13)),
        );
        x += spacing;
    }
    painter.rect_stroke(
        r,
        0.,
        Stroke::new(1., rgb(100, 115, 112)),
        StrokeKind::Inside,
    );
}
pub fn text(ui: &Ui, at: Pos2, label: &str, size: f32, bold: bool, color: Color32, align: Align2) {
    ui.painter().text(
        at,
        align,
        label,
        FontId::new(
            size,
            if bold {
                FontFamily::Name("rack-bold".into())
            } else {
                FontFamily::Proportional
            },
        ),
        color,
    );
}
fn beveled(ui: &Ui, r: Rect, selected: bool, down: bool) {
    let stops: &[(f32, Color32)] = if selected {
        &[
            (0., rgb(255, 226, 216)),
            (0.55, rgb(249, 101, 84)),
            (1., rgb(251, 166, 153)),
        ]
    } else {
        &[
            (0., rgb(249, 250, 247)),
            (0.45, rgb(216, 222, 220)),
            (0.65, rgb(155, 169, 169)),
            (1., rgb(184, 195, 192)),
        ]
    };
    gradient(ui, r, stops, false);
    let p = ui.painter();
    p.rect_stroke(r, 2., Stroke::new(1., rgb(81, 94, 93)), StrokeKind::Inside);
    let hi = if down {
        rgb(48, 65, 61)
    } else {
        rgb(255, 255, 255)
    };
    p.line_segment(
        [
            r.left_bottom() + egui::vec2(1., -1.),
            r.left_top() + egui::vec2(1., 1.),
        ],
        Stroke::new(0.7, hi),
    );
    p.line_segment(
        [
            r.left_top() + egui::vec2(1., 1.),
            r.right_top() + egui::vec2(-1., 1.),
        ],
        Stroke::new(0.7, hi),
    );
}
pub fn button(ui: &mut Ui, r: Rect, id: &str, label: &str, selected: bool, font: f32) -> Response {
    let response = ui.interact(r, ui.make_persistent_id(id), Sense::click());
    beveled(ui, r, selected, response.is_pointer_button_down_on());
    text(
        ui,
        r.center(),
        label,
        font,
        true,
        if selected {
            rgb(101, 43, 38)
        } else {
            rgb(23, 35, 35)
        },
        Align2::CENTER_CENTER,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            r.expand(1.),
            2.,
            Stroke::new(1., rgb(255, 189, 79)),
            StrokeKind::Outside,
        );
    }
    response
}
pub fn light(
    ui: &Ui,
    center: Pos2,
    size: f32,
    on: bool,
    active: Color32,
    off: Color32,
    square: bool,
) {
    let p = ui.painter();
    let c = if on { active } else { off };
    if on {
        p.circle_filled(
            center,
            size * 0.8,
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 45),
        );
    }
    if square {
        p.rect_filled(Rect::from_center_size(center, Vec2::splat(size)), 0., c);
    } else {
        p.circle_filled(center, size * 0.5, c);
    }
}
const MASKS: [u8; 10] = [
    0b0111111, 0b0000110, 0b1011011, 0b1001111, 0b1100110, 0b1101101, 0b1111101, 0b0000111,
    0b1111111, 0b1101111,
];
pub fn digits(ui: &Ui, r: Rect, value: usize, digits: usize) {
    let p = ui.painter();
    p.rect_filled(r, 0., rgb(27, 8, 6));
    p.rect_stroke(r, 0., Stroke::new(1., rgb(40, 55, 57)), StrokeKind::Outside);
    let width = (digits as f32 * 10. - 1.).max(1.);
    let sx = r.width() / width;
    let sy = r.height() / 16.;
    let mut v = value;
    let mut vals = vec![0; digits];
    for d in vals.iter_mut().rev() {
        *d = v % 10;
        v /= 10;
    }
    for (i, d) in vals.into_iter().enumerate() {
        let base = egui::pos2(r.left() + i as f32 * 10. * sx, r.top());
        let segs = [
            (2., 0., 5., 2.),
            (7., 1., 2., 6.),
            (7., 8., 2., 6.),
            (2., 14., 5., 2.),
            (0., 8., 2., 6.),
            (0., 1., 2., 6.),
            (2., 7., 5., 2.),
        ];
        for (j, (x, y, w, h)) in segs.into_iter().enumerate() {
            let c = if MASKS[d] & (1 << j) != 0 {
                rgb(255, 52, 40)
            } else {
                rgb(38, 11, 8)
            };
            let (x, y, w, h) = (base.x + x * sx, base.y + y * sy, w * sx, h * sy);
            let points = if w > h {
                vec![
                    egui::pos2(x, y + h / 2.),
                    egui::pos2(x + w * 0.2, y),
                    egui::pos2(x + w * 0.8, y),
                    egui::pos2(x + w, y + h / 2.),
                    egui::pos2(x + w * 0.8, y + h),
                    egui::pos2(x + w * 0.2, y + h),
                ]
            } else {
                vec![
                    egui::pos2(x + w / 2., y),
                    egui::pos2(x + w, y + h * 0.15),
                    egui::pos2(x + w, y + h * 0.85),
                    egui::pos2(x + w / 2., y + h),
                    egui::pos2(x, y + h * 0.85),
                    egui::pos2(x, y + h * 0.15),
                ]
            };
            p.add(Shape::convex_polygon(points, c, Stroke::NONE));
        }
    }
}
pub fn knob(ui: &Ui, r: Rect, normalized: f32, kind: KnobKind) {
    let p = ui.painter();
    let c = r.center();
    let radius = r.width().min(r.height()) * 0.5;
    let (outer, mid, highlight, marker, ticks) = match kind {
        KnobKind::Dark => (
            rgb(18, 46, 52),
            rgb(53, 75, 76),
            rgb(168, 180, 172),
            rgb(33, 52, 46),
            false,
        ),
        KnobKind::Synth => (
            rgb(70, 84, 76),
            rgb(180, 190, 178),
            rgb(255, 255, 255),
            rgb(89, 101, 90),
            true,
        ),
        KnobKind::Red => (
            rgb(56, 28, 24),
            rgb(214, 48, 38),
            rgb(248, 81, 65),
            rgb(35, 41, 33),
            false,
        ),
        KnobKind::Cream => (
            rgb(53, 62, 48),
            rgb(227, 223, 184),
            rgb(255, 248, 207),
            rgb(44, 55, 40),
            false,
        ),
        KnobKind::Drum909 => (
            rgb(64, 79, 62),
            rgb(123, 136, 112),
            rgb(184, 190, 165),
            rgb(213, 141, 77),
            true,
        ),
    };
    if ticks {
        for i in 0..12 {
            let a = 2. * PI * i as f32 / 12.;
            let d = egui::vec2(a.sin(), -a.cos());
            p.line_segment(
                [c + d * radius * 0.76, c + d * radius * 0.99],
                Stroke::new((radius * 0.1).max(0.5), outer),
            );
        }
    }
    let rad = radius * if ticks { 0.68 } else { 0.95 };
    p.circle_filled(
        c + egui::vec2(0.7, 0.8),
        rad,
        Color32::from_black_alpha(100),
    );
    p.circle_filled(c, rad, outer);
    for i in (0..10).rev() {
        let t = i as f32 / 10.;
        let offset = egui::vec2(-rad * 0.2, -rad * 0.2) * (1. - t);
        p.circle_filled(c + offset, rad * (0.22 + 0.72 * t), mix(highlight, mid, t));
    }
    let a = (-135. + 270. * normalized.clamp(0., 1.)) * PI / 180.;
    let d = egui::vec2(a.sin(), -a.cos());
    p.line_segment(
        [c + d * rad * 0.1, c + d * rad * 0.72],
        Stroke::new((rad * 0.14).max(1.), marker),
    );
}
pub fn fader(ui: &Ui, r: Rect, normalized: f32) {
    let p = ui.painter();
    let center = r.center().x;
    let mut y = r.top();
    while y <= r.bottom() {
        p.line_segment(
            [egui::pos2(r.left(), y), egui::pos2(r.right(), y)],
            Stroke::new(1., rgb(197, 211, 202)),
        );
        y += 6.;
    }
    p.rect_filled(
        Rect::from_center_size(r.center(), egui::vec2(3., r.height())),
        0.,
        rgb(36, 59, 60),
    );
    let h = 11_f32.min(r.height());
    let cy = egui::lerp(
        (r.bottom() - h / 2.)..=(r.top() + h / 2.),
        normalized.clamp(0., 1.),
    );
    let thumb = Rect::from_center_size(egui::pos2(center, cy), egui::vec2(r.width(), h));
    gradient(
        ui,
        thumb,
        &[
            (0., rgb(38, 54, 49)),
            (0.35, rgb(38, 54, 49)),
            (0.5, rgb(155, 166, 159)),
            (0.65, rgb(32, 53, 44)),
            (1., rgb(32, 53, 44)),
        ],
        false,
    );
    p.rect_stroke(
        thumb,
        1.,
        Stroke::new(1., rgb(41, 63, 57)),
        StrokeKind::Inside,
    );
}
#[cfg(test)]
mod tests {
    #[test]
    fn digit_masks() {
        assert_eq!(super::MASKS[0].count_ones(), 6);
        assert_eq!(super::MASKS[8].count_ones(), 7);
        assert_eq!(super::MASKS[1].count_ones(), 2);
    }
}
