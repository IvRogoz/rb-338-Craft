use super::{rect, skin, NativeApp};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind};
use skin::{KnobKind, Surface};

const GREEN: Color32 = Color32::from_rgb(75, 244, 54);
const OFF: Color32 = Color32::from_rgb(24, 62, 44);
const YELLOW: Color32 = Color32::from_rgb(252, 223, 66);

fn label(ui: &egui::Ui, r: Rect, s: &str, size: f32) {
    skin::text(
        ui,
        r.center(),
        s,
        size,
        false,
        skin::LIGHT,
        Align2::CENTER_CENTER,
    );
}
fn action(
    app: &mut NativeApp,
    ui: &mut egui::Ui,
    r: Rect,
    id: &str,
    text: &str,
    on: bool,
    size: f32,
) {
    let lamp = id == "play" || id == "record";
    let selected = on && (id.starts_with("pattern:") || id.starts_with("bank:"));
    if skin::button(ui, r, id, if lamp { "" } else { text }, selected, size).clicked() {
        app.c.action(id);
    }
    if lamp {
        let color = match (id, on) {
            ("play", true) => GREEN,
            ("record", true) => Color32::from_rgb(255, 51, 37),
            ("record", false) => Color32::from_rgb(113, 44, 34),
            _ => Color32::from_rgb(59, 81, 82),
        };
        skin::text(
            ui,
            r.center(),
            text,
            size,
            true,
            color,
            Align2::CENTER_CENTER,
        );
    }
}
fn indicator(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect, id: &str, on: bool) {
    skin::light(
        ui,
        r.center(),
        r.width().min(r.height()),
        on,
        GREEN,
        OFF,
        true,
    );
    if ui.interact(r, ui.id().with(id), Sense::click()).clicked() {
        app.c.action(id);
    }
}
fn meter(ui: &egui::Ui, r: Rect, value: f32, color: Color32) {
    ui.painter()
        .rect_filled(r, 0.0, Color32::from_rgb(38, 60, 44));
    let h = r.height() * value.clamp(0.0, 1.0);
    ui.painter().rect_filled(
        Rect::from_min_max(Pos2::new(r.left(), r.bottom() - h), r.max),
        0.0,
        color,
    );
}
fn stepper(
    app: &mut NativeApp,
    ui: &mut egui::Ui,
    r: Rect,
    name: &str,
    value: usize,
    count: usize,
) {
    skin::digits(
        ui,
        rect(r, 0., 0., count as f32 * 10. - 1., 16.),
        value,
        count,
    );
    for (dy, sign, glyph) in [(0., 1, "▲"), (8., -1, "▼")] {
        let id = format!("{name}:{sign}");
        action(
            app,
            ui,
            rect(r, count as f32 * 10. + 1., dy, 8., 8.),
            &id,
            glyph,
            false,
            5.,
        );
    }
}
fn header(ui: &egui::Ui, r: Rect, title: &str, color: Color32) {
    ui.painter().rect_filled(r, 0., color);
    ui.painter().rect_stroke(
        r,
        0.,
        Stroke::new(1., Color32::from_rgb(48, 67, 74)),
        StrokeKind::Inside,
    );
    label(ui, r, title, if title == "MIX" { 10. } else { 9. });
}
fn knob(
    app: &mut NativeApp,
    ui: &mut egui::Ui,
    r: Rect,
    label_text: &str,
    key: &str,
    limits: (f32, f32),
) {
    if key.starts_with("channel:") || key.starts_with("fx:") {
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.;
            let d = egui::vec2(a.sin(), -a.cos());
            ui.painter().line_segment(
                [r.center() + d * 10.5, r.center() + d * 12.5],
                Stroke::new(0.7, Color32::from_rgb(199, 209, 195)),
            );
        }
    }
    app.knob(ui, r, label_text, key, limits, KnobKind::Dark);
    label(
        ui,
        rect(r, -4., r.height() + 3., r.width() + 8., 7.),
        label_text,
        if key.starts_with("channel:") { 7. } else { 5. },
    );
}
fn transport(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect) {
    let song = app.display.clone();
    skin::surface(ui, r, Surface::Steel);
    knob(
        app,
        ui,
        rect(r, 10., 12., 19., 19.),
        "SHUFFLE",
        "shuffle",
        (0., 60.),
    );
    for (x, name, id, on) in [
        (41., "Sync", "midi_sync", app.c.midi_sync),
        (84., "MIDI", "midi_modal", app.midi.is_some()),
        (127., "Pattern", "pattern_mode", !song.song_mode),
        (187., "Song mode", "song_mode", song.song_mode),
    ] {
        skin::light(
            ui,
            Pos2::new(r.left() + x + 2., r.top() + 7.),
            4.,
            on,
            GREEN,
            OFF,
            false,
        );
        label(
            ui,
            rect(
                r,
                x + 7.,
                2.,
                if name == "Song mode" { 48. } else { 36. },
                10.,
            ),
            name,
            9.,
        );
        if ui
            .interact(
                rect(r, x, 2., if name == "Song mode" { 55. } else { 43. }, 12.),
                ui.id().with(id),
                Sense::click(),
            )
            .clicked()
        {
            if id == "midi_modal" {
                app.modal = true;
                app.library = super::platform::library_songs().unwrap_or_default();
            } else {
                app.c.action(id);
            }
        }
    }
    let looping = song.looping;
    skin::light(
        ui,
        Pos2::new(r.left() + 317., r.top() + 6.),
        6.,
        looping,
        YELLOW,
        OFF,
        false,
    );
    label(ui, rect(r, 337., 1., 56., 10.), "Loop", 9.);
    if ui
        .interact(
            rect(r, 315., 1., 92., 12.),
            ui.id().with("loop"),
            Sense::click(),
        )
        .clicked()
    {
        app.c.action("loop");
    }
    for (i, (id, glyph, on)) in [
        ("play", "▶", app.c.playing),
        ("stop", "■", false),
        ("transport:-10", "◀◀", false),
        ("transport:10", "▶▶", false),
        ("record", "●", app.c.recording),
    ]
    .into_iter()
    .enumerate()
    {
        action(
            app,
            ui,
            rect(r, 97. + i as f32 * 31.6, 24., 31.6, 19.),
            id,
            glyph,
            on,
            8.,
        );
    }
    for (x, name, id, value) in [
        (42., "Tempo", "tempo", song.bpm.round() as usize),
        (272., "Bar", "transport", app.c.position / 16 + 1),
        (330., "Start", "start", song.start + 1),
        (373., "Length", "length", song.length),
    ] {
        label(ui, rect(r, x, 15., 31., 9.), name, 8.);
        stepper(app, ui, rect(r, x, 25., 43., 16.), id, value, 3);
    }
    for i in 0..4 {
        let center = Pos2::new(r.left() + 447.5 + i as f32 * 14., r.top() + 7.5);
        ui.painter().add(egui::Shape::ellipse_filled(
            center + egui::vec2(1., 1.),
            egui::vec2(5.5, 6.5),
            Color32::from_rgb(23, 45, 51),
        ));
        ui.painter().add(egui::Shape::ellipse_filled(
            center,
            egui::vec2(5.5, 6.5),
            Color32::from_rgb(144, 166, 171),
        ));
    }
    skin::text(
        ui,
        Pos2::new(r.right() - 4., r.top()),
        "ReBirth",
        20.,
        false,
        Color32::from_rgb(175, 192, 194),
        Align2::RIGHT_TOP,
    );
    ui.painter().line_segment(
        [
            Pos2::new(r.left() + 440., r.top() + 18.),
            Pos2::new(r.right(), r.top() + 18.),
        ],
        Stroke::new(0.6, Color32::from_rgb(175, 192, 194)),
    );
    let origin = Pos2::new(r.left() + 440., r.top() + 18.);
    for (offset, color) in [
        (egui::vec2(1., 1.), Color32::from_rgb(23, 45, 51)),
        (egui::vec2(-0.5, -0.5), Color32::from_rgb(210, 220, 208)),
        (egui::Vec2::ZERO, Color32::from_rgb(175, 192, 194)),
    ] {
        let source = ui.painter().layout_no_wrap(
            "RB-338".into(),
            egui::FontId::new(30., egui::FontFamily::Name("rack-bold".into())),
            color,
        );
        let mut galley = (*source).clone();
        // Keep glyph UVs in font-atlas coordinates until egui's end-of-frame tessellation.
        for placed in &mut galley.rows {
            placed.pos.x *= 1.43;
            let row = std::sync::Arc::make_mut(&mut placed.row);
            row.size.x *= 1.43;
            for vertex in &mut row.visuals.mesh.vertices {
                vertex.pos.x *= 1.43;
            }
            row.visuals.mesh_bounds.min.x *= 1.43;
            row.visuals.mesh_bounds.max.x *= 1.43;
        }
        galley.rect.min.x *= 1.43;
        galley.rect.max.x *= 1.43;
        galley.mesh_bounds.min.x *= 1.43;
        galley.mesh_bounds.max.x *= 1.43;
        ui.painter()
            .galley(origin + offset, std::sync::Arc::new(galley), color);
    }
}
fn pattern(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect, t: usize) {
    let song = app.display.clone();
    skin::surface(ui, r, Surface::Steel);
    header(
        ui,
        rect(r, 3., 3., 62., 13.),
        "PATTERN",
        Color32::from_rgb(143, 37, 49),
    );
    indicator(
        app,
        ui,
        rect(r, 6., 5., 7., 7.),
        &format!("enable:{t}"),
        song.enabled[t],
    );
    let slot = song.slots[t];
    let bank = app.c.bank[t];
    for i in 0..8 {
        action(
            app,
            ui,
            rect(
                r,
                3. + (i % 4) as f32 * 16.,
                23. + (i / 4) as f32 * 15.,
                13.,
                13.,
            ),
            &format!("pattern:{t}:{i}"),
            &(i + 1).to_string(),
            slot % 8 == i && slot / 8 == bank,
            8.,
        );
    }
    label(ui, rect(r, 4., 57., 22., 6.), "BANK", 6.);
    for (i, s) in ["A", "B", "C", "D"].into_iter().enumerate() {
        action(
            app,
            ui,
            rect(r, 3. + i as f32 * 16., 65., 13., 13.),
            &format!("bank:{t}:{i}"),
            s,
            bank == i,
            8.,
        );
    }
    label(ui, rect(r, 4., 88., 27., 6.), "SHUFFLE", 5.);
    label(ui, rect(r, 38., 88., 24., 6.), "STEPS", 5.);
    action(
        app,
        ui,
        rect(r, 4., 96., 14., 15.),
        &format!("shuffle_track:{t}"),
        "",
        song.shuffle_tracks[t],
        5.,
    );
    skin::digits(
        ui,
        rect(r, 36., 96., 19., 16.),
        song.patterns[slot].lengths[t],
        2,
    );
    for (dy, id, glyph) in [
        (0., format!("steps_up:{t}"), "▲"),
        (8., format!("steps_down:{t}"), "▼"),
    ] {
        action(
            app,
            ui,
            rect(r, 57., 96. + dy, 7., 8.),
            &id,
            glyph,
            false,
            5.,
        );
    }
}
fn mixer(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect, t: usize, peak: f32) {
    let song = app.display.clone();
    skin::surface(ui, r, Surface::Steel);
    header(
        ui,
        rect(r, 3., 3., 62., 13.),
        "MIX",
        Color32::from_rgb(150, 141, 48),
    );
    indicator(
        app,
        ui,
        rect(r, 6., 5., 7., 7.),
        &format!("mute:{t}"),
        !song.channels[t].mute,
    );
    meter(ui, rect(r, 60., 5., 4., 9.), peak, GREEN);
    knob(
        app,
        ui,
        rect(r, 8., 24., 19., 19.),
        "PAN",
        &format!("channel:{t}:pan"),
        (-1., 1.),
    );
    app.fader(
        ui,
        rect(r, 9., 57., 15., 50.),
        &format!("channel:{t}:level"),
        (0., 1.),
    );
    knob(
        app,
        ui,
        rect(r, 41., 81., 19., 19.),
        "DELAY",
        &format!("channel:{t}:delay"),
        (0., 1.),
    );
    for (y, name, id, on) in [
        (20., "DIST", "dist", song.channels[t].distortion),
        (38., "PCF", "filter", song.channels[t].filter),
        (56., "COMP", "comp", song.channels[t].compressor),
    ] {
        action(
            app,
            ui,
            rect(r, 43., y, 12., 6.),
            &format!("{id}:{t}"),
            "",
            on,
            5.,
        );
        skin::light(
            ui,
            Pos2::new(r.left() + 59., r.top() + y + 3.),
            3.,
            on,
            YELLOW,
            OFF,
            false,
        );
        label(ui, rect(r, 35., y + 8., 28., 7.), name, 6.);
    }
}
fn master(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect, metrics: [f32; 11]) {
    let song = app.display.clone();
    let heights = [98., 111., 93., 65., 93.];
    let names = ["MASTER", "PCF", "DELAY", "DIST", "COMP"];
    let mut y = 0.;
    for i in 0..5 {
        let b = rect(r, 0., y, 82., heights[i]);
        y += heights[i];
        skin::surface(ui, b, Surface::Steel);
        header(
            ui,
            rect(b, 2., 3., 76., 13.),
            names[i],
            if i == 0 {
                Color32::from_rgb(150, 141, 48)
            } else {
                Color32::from_rgb(37, 60, 120)
            },
        );
        if i > 0 {
            let (field, on) = match i {
                1 => ("filter_enabled", song.fx.filter_enabled),
                2 => ("delay_enabled", song.fx.delay_enabled),
                3 => ("distortion_enabled", song.fx.distortion_enabled),
                _ => ("compressor_enabled", song.fx.compressor_enabled),
            };
            indicator(
                app,
                ui,
                rect(b, 5., 6., 7., 7.),
                &format!("fx_toggle:{field}"),
                on,
            );
            meter(ui, rect(b, 71., 5., 4., 9.), metrics[i + 6], GREEN);
        }
        match i {
            0 => {
                meter(ui, rect(b, 19., 27., 5., 50.), metrics[0], GREEN);
                meter(ui, rect(b, 58., 27., 5., 50.), metrics[1], GREEN);
                app.fader(ui, rect(b, 33., 27., 15., 52.), "fx:master", (0., 1.5));
                label(
                    ui,
                    rect(b, 3., 24., 15., 55.),
                    "Clip\n-6\n-12\n-24\n-36",
                    5.,
                );
                label(
                    ui,
                    rect(b, 64., 24., 15., 55.),
                    "Clip\n-6\n-12\n-24\n-36",
                    5.,
                );
                label(ui, rect(b, 23., 84., 24., 8.), "COMP", 6.);
                action(
                    app,
                    ui,
                    rect(b, 49., 85., 12., 6.),
                    "master_comp",
                    "",
                    song.fx.compressor_master,
                    5.,
                );
            }
            1 => {
                stepper(
                    app,
                    ui,
                    rect(b, 8., 23., 30., 16.),
                    "fx_step:filter_pattern",
                    song.fx.filter_pattern,
                    2,
                );
                action(
                    app,
                    ui,
                    rect(b, 50., 23., 6., 16.),
                    "filter_mode",
                    "",
                    false,
                    5.,
                );
                label(ui, rect(b, 59., 23., 16., 16.), "BP\nLP", 7.);
                label(ui, rect(b, 7., 41., 30., 7.), "PATTERN", 7.);
                label(ui, rect(b, 47., 41., 25., 7.), "MODE", 7.);
                for (j, (n, k, limits)) in [
                    ("FREQ", "filter_cutoff", (100., 8000.)),
                    ("Q", "filter_q", (0.5, 15.)),
                    ("AMT", "filter_amount", (0., 1.)),
                    ("DEC", "filter_decay", (0.1, 10.)),
                ]
                .into_iter()
                .enumerate()
                {
                    let x = 4. + j as f32 * 18.;
                    app.fader(ui, rect(b, x, 56., 15., 42.), &format!("fx:{k}"), limits);
                    label(ui, rect(b, x, 101., 16., 7.), n, 7.);
                }
            }
            2 => {
                stepper(
                    app,
                    ui,
                    rect(b, 8., 23., 30., 16.),
                    "fx_step:delay_steps",
                    song.fx.delay_steps,
                    2,
                );
                action(
                    app,
                    ui,
                    rect(b, 50., 23., 6., 16.),
                    "fx_toggle:delay_triplet",
                    "",
                    song.fx.delay_triplet,
                    5.,
                );
                label(ui, rect(b, 59., 23., 16., 16.), "3\n♩", 8.);
                label(ui, rect(b, 7., 41., 28., 7.), "STEPS", 7.);
                knob(
                    app,
                    ui,
                    rect(b, 12., 64., 19., 19.),
                    "PAN",
                    "fx:delay_pan",
                    (-1., 1.),
                );
                knob(
                    app,
                    ui,
                    rect(b, 49., 64., 19., 19.),
                    "F.BACK",
                    "fx:delay_feedback",
                    (0., 0.98),
                );
            }
            3 => {
                knob(
                    app,
                    ui,
                    rect(b, 12., 30., 19., 19.),
                    "AMOUNT",
                    "fx:distortion",
                    (0., 1.),
                );
                knob(
                    app,
                    ui,
                    rect(b, 53., 30., 19., 19.),
                    "SHAPE",
                    "fx:distortion_shape",
                    (0., 1.),
                );
            }
            _ => {
                meter(ui, rect(b, 8., 26., 64., 7.), metrics[6] / 40., YELLOW);
                label(
                    ui,
                    rect(b, 7., 36., 68., 6.),
                    "-40  -20    0   +20  +40",
                    6.,
                );
                knob(
                    app,
                    ui,
                    rect(b, 12., 57., 19., 19.),
                    "RATIO",
                    "fx:compressor_ratio",
                    (1., 20.),
                );
                knob(
                    app,
                    ui,
                    rect(b, 49., 57., 19., 19.),
                    "THRES",
                    "fx:compressor_threshold",
                    (-40., 0.),
                );
            }
        }
    }
}
pub fn draw(app: &mut NativeApp, ui: &mut egui::Ui, canvas: Rect) {
    skin::surface(ui, rect(canvas, 0., 0., 16., 527.), Surface::Wood);
    skin::surface(ui, rect(canvas, 608., 0., 16., 527.), Surface::Wood);
    let launcher = rect(canvas, 16., 0., 592., 17.);
    skin::surface(ui, launcher, Surface::Steel);
    skin::text(
        ui,
        Pos2::new(launcher.left() + 5., launcher.top() + 4.),
        "SONG / FILE · LIBRARY",
        8.,
        false,
        skin::LIGHT,
        Align2::LEFT_TOP,
    );
    if ui
        .interact(launcher, ui.id().with("song_launcher"), Sense::click())
        .clicked()
    {
        app.modal = true;
        app.library = super::platform::library_songs().unwrap_or_default();
    }
    transport(app, ui, rect(canvas, 16., 17., 592., 50.));
    let metrics = app.c.audio.snapshot().metrics;
    for t in 0..4 {
        let row = rect(canvas, 16., 67. + t as f32 * 115., 510., 115.);
        pattern(app, ui, rect(row, 0., 0., 70., 115.), t);
        let divider = rect(row, 70., 0., 4., 115.);
        ui.painter().rect_filled(
            divider,
            0.,
            if app.c.track == t {
                Color32::from_rgb(210, 55, 48)
            } else {
                Color32::from_rgb(45, 65, 68)
            },
        );
        let device = rect(row, 74., 0., 366., 115.);
        if t < 2 {
            super::instruments::synth(app, ui, device, t);
        } else {
            super::instruments::drums(app, ui, device, t - 2);
        }
        mixer(app, ui, rect(row, 440., 0., 70., 115.), t, metrics[t + 2]);
    }
    master(app, ui, rect(canvas, 526., 67., 82., 460.), metrics);
    if !app.c.status.is_empty() {
        let status = rect(canvas, 16., 509., 592., 18.);
        ui.painter()
            .rect_filled(status, 0., Color32::from_rgb(63, 85, 88));
        skin::text(
            ui,
            Pos2::new(status.left() + 4., status.top() + 4.),
            &app.c.status,
            10.,
            false,
            Color32::from_rgb(255, 240, 186),
            Align2::LEFT_TOP,
        );
    }
}
