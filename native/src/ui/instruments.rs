use super::{
    rect,
    skin::{self, KnobKind, Surface, INK, LIGHT},
    NativeApp,
};
use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind};
use rb_model::{DRUM_IDS, DRUM_IDS_B};

fn label(ui: &egui::Ui, r: Rect, value: &str, size: f32, color: Color32) {
    skin::text(
        ui,
        r.center(),
        value,
        size,
        true,
        color,
        Align2::CENTER_CENTER,
    );
}
fn click(ui: &mut egui::Ui, r: Rect, id: String) -> bool {
    ui.interact(r, ui.make_persistent_id(id), Sense::click())
        .clicked()
}
fn gradient(ui: &egui::Ui, r: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}
fn led(ui: &egui::Ui, x: f32, y: f32, on: bool) {
    skin::light(
        ui,
        pos2(x, y),
        4.,
        on,
        Color32::from_rgb(255, 48, 34),
        Color32::from_rgb(83, 25, 23),
        false,
    );
}
fn switch(ui: &mut egui::Ui, r: Rect, id: String, on: bool) -> bool {
    let hit = click(ui, r, id);
    ui.painter()
        .rect_filled(r, 0., Color32::from_rgb(223, 231, 217));
    ui.painter().rect_stroke(
        r,
        0.,
        Stroke::new(0.7, Color32::from_rgb(69, 88, 74)),
        StrokeKind::Inside,
    );
    let inner = Rect::from_min_max(r.min + vec2(1., 1.), r.max - vec2(1., 1.));
    ui.painter().line_segment(
        [inner.left_top(), inner.left_bottom()],
        Stroke::new(1., Color32::WHITE),
    );
    if on {
        ui.painter().line_segment(
            [inner.right_top(), inner.right_bottom()],
            Stroke::new(1., Color32::from_rgb(109, 134, 110)),
        );
    }
    hit
}

pub fn synth(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect, t: usize) {
    skin::surface(ui, r, Surface::Silver);
    let p = ui.painter();
    p.line_segment(
        [r.min + vec2(0., 43.), r.min + vec2(364., 43.)],
        Stroke::new(2., Color32::from_rgb(158, 170, 159)),
    );
    label(ui, rect(r, 7., 2., 55., 8.), "WAVEFORM", 7., INK);
    let wave_color = Color32::from_rgb(38, 55, 44);
    let wave_origin = r.min + vec2(7., 12.);
    // Decompose the web waveform's CSS polygon into three bars and two convex bands.
    for (x, y, w, h) in [
        (0., 0., 2.4, 10.),
        (6.9, 0., 2.4, 10.),
        (13.6, 0., 2.4, 10.),
    ] {
        ui.painter().rect_filled(
            Rect::from_min_size(wave_origin + vec2(x, y), vec2(w, h)),
            0.,
            wave_color,
        );
    }
    for (left, right) in [(2.4, 6.9), (9.3, 13.6)] {
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                wave_origin + vec2(left, 0.),
                wave_origin + vec2(right, 7.5),
                wave_origin + vec2(right, 10.),
                wave_origin + vec2(left, 2.5),
            ],
            wave_color,
            Stroke::NONE,
        ));
    }
    label(ui, rect(r, 43., 14., 13., 14.), "⊓", 12., INK);
    let wave = rect(r, 24., 12., 17., 10.);
    let is_square = app.display.synth[t].wave == "square";
    let wave_clicked = click(ui, wave, format!("wave:{t}"));
    ui.painter()
        .rect_filled(wave, 0., Color32::from_rgb(26, 37, 31));
    ui.painter().rect_filled(
        rect(wave, if is_square { 11. } else { 0. }, 0., 6., 10.),
        0.,
        Color32::from_rgb(137, 150, 140),
    );
    ui.painter().rect_stroke(
        wave,
        0.,
        Stroke::new(0.7, Color32::from_rgb(115, 129, 122)),
        StrokeKind::Inside,
    );
    if wave_clicked {
        app.c.track = t;
        app.c.action(&format!("wave:{t}"));
    }
    for (i, (name, key, min, max)) in [
        ("TUNE", "tune", -12., 12.),
        ("CUTOFF", "cutoff", 100., 6000.),
        ("RESO", "resonance", 0., 1.),
        ("ENV.MOD", "envelope", 0., 5000.),
        ("DECAY", "decay", 0.05, 0.8),
        ("ACCENT", "accent", 0., 1.),
    ]
    .iter()
    .enumerate()
    {
        let x = 68. + i as f32 * 33.;
        label(ui, rect(r, x, 1., 33., 8.), name, 7., INK);
        app.knob(
            ui,
            rect(r, x + 1., 11., 31., 30.),
            name,
            &format!("synth:{t}:{key}"),
            (*min, *max),
            KnobKind::Synth,
        );
    }
    label(ui, rect(r, 319., 7., 37., 8.), "EDIT STEP", 7., INK);
    skin::digits(ui, rect(r, 328., 16., 19., 16.), app.c.edit[t] + 1, 2);
    let mode = rect(r, 6., 47., 37., 62.);
    ui.painter()
        .rect_filled(mode, 0., Color32::from_rgb(17, 25, 22));
    label(ui, rect(mode, 1., 2., 35., 15.), "PITCH\nMODE", 7., LIGHT);
    let mode_on = app.c.pitch_mode[t];
    led(ui, mode.left() + 18., mode.top() + 20., mode_on);
    if switch(
        ui,
        rect(mode, 11., 26., 15., 8.),
        format!("pitchmode:{t}"),
        mode_on,
    ) {
        app.c.track = t;
        app.c.action(&format!("pitchmode:{t}"));
    }
    ui.painter().rect_filled(
        rect(mode, 0., 40., 37., 22.),
        0.,
        Color32::from_rgb(230, 234, 226),
    );
    label(ui, rect(mode, 0., 40., 37., 8.), "CLEAR", 6., INK);
    if switch(
        ui,
        rect(mode, 10., 51., 15., 7.),
        format!("clear:{t}"),
        false,
    ) {
        app.c.track = t;
        app.c.action(&format!("clear:{t}"));
    }
    let keys = rect(r, 44., 51., 170., 58.);
    ui.painter()
        .rect_filled(keys, 0., Color32::from_rgb(246, 247, 240));
    ui.painter()
        .rect_stroke(keys, 0., Stroke::new(2., INK), StrokeKind::Inside);
    let note = app.c.song.patterns[app.c.song.slots[t]].notes[t][app.c.edit[t]].clone();
    let offsets = [
        0., 15., 21., 36., 42., 63., 78., 84., 99., 105., 120., 126., 147.,
    ];
    for sharp in [false, true] {
        for (i, x) in offsets.iter().enumerate() {
            if [1, 3, 6, 8, 10].contains(&i) != sharp {
                continue;
            }
            let key = rect(
                keys,
                2. + x,
                2.,
                if sharp {
                    14.
                } else if i == 12 {
                    19.
                } else {
                    21.
                },
                if sharp { 33. } else { 54. },
            );
            if sharp {
                ui.painter()
                    .rect_filled(key, 1., Color32::from_rgb(16, 25, 20));
            }
            let selected = (i < 12 && note.note % 12 == i as u8 && !(i == 0 && note.note >= 48))
                || (i == 12 && note.note % 12 == 0 && note.note >= 48);
            let light_y = if sharp {
                key.top() + 4.
            } else {
                key.bottom() - 21.
            };
            led(
                ui,
                key.left() + if sharp { 7. } else { 10. },
                light_y,
                selected,
            );
            let sub = rect(
                key,
                if sharp { 3. } else { 6. },
                key.height() - 18.,
                if sharp { 8. } else { 9. },
                15.,
            );
            ui.painter()
                .rect_filled(sub, 0., Color32::from_rgb(218, 228, 214));
            ui.painter().rect_stroke(
                sub,
                0.,
                Stroke::new(0.7, Color32::from_rgb(69, 88, 74)),
                StrokeKind::Inside,
            );
            if click(ui, key, format!("pitch-area:{t}:{i}")) {
                app.c.track = t;
                app.c.action(&format!("pitchkey:{t}:{i}"));
            }
        }
    }
    let sc = rect(r, 214., 47., 144., 62.);
    ui.painter().rect_filled(
        rect(sc, 0., 0., 101., 25.),
        0.,
        Color32::from_rgb(237, 240, 231),
    );
    label(ui, rect(sc, 0., 0., 28., 24.), "♪", 22., INK);
    led(ui, sc.left() + 38., sc.top() + 13., note.gate);
    if click(ui, rect(sc, 0., 0., 50., 25.), format!("note:{t}")) {
        app.c.track = t;
        app.c.action(&format!("note:{t}"));
    }
    // Paint the rest as a vector so it does not depend on a musical-symbol font.
    let rest = sc.min + vec2(59., 3.);
    ui.painter().add(egui::Shape::line(
        vec![
            rest + vec2(4., 0.),
            rest + vec2(8., 5.),
            rest + vec2(1., 10.),
            rest + vec2(8., 15.),
            rest + vec2(4., 18.),
            rest + vec2(5., 21.),
        ],
        Stroke::new(2.5, INK),
    ));
    let _ = switch(
        ui,
        rect(sc, 80., 10., 14., 7.),
        format!("sustain-display:{t}"),
        false,
    );
    for (i, (title, key, on)) in [
        ("DOWN", "down", note.note < 36),
        ("UP", "up", note.note >= 48),
        ("ACCENT", "accent", note.accent),
        ("SLIDE", "slide", note.slide),
    ]
    .iter()
    .enumerate()
    {
        let x = i as f32 * 25.;
        let area = rect(sc, x, 25., 25., 37.);
        ui.painter()
            .rect_filled(area, 0., Color32::from_rgb(237, 240, 231));
        ui.painter().rect_filled(
            rect(area, 0., 0., 25., 8.),
            0.,
            Color32::from_rgb(18, 27, 21),
        );
        label(ui, rect(area, 0., 0., 25., 8.), title, 6., LIGHT);
        led(ui, area.center().x, area.top() + 17., *on);
        if switch(ui, rect(area, 9., 19., 8., 15.), format!("{key}:{t}"), *on) {
            app.c.track = t;
            app.c.action(&format!("{key}:{t}"));
        }
    }
    for (y, title, key) in [(0., "BACK", "back"), (31., "STEP", "next")] {
        let b = rect(sc, 101., y, 43., 31.);
        ui.painter()
            .rect_filled(b, 0., Color32::from_rgb(237, 240, 231));
        label(ui, rect(b, 0., 0., 43., 9.), title, 7., INK);
        if switch(ui, rect(b, 12., 17., 19., 10.), format!("{key}:{t}"), false) {
            app.c.track = t;
            app.c.action(&format!("{key}:{t}"));
        }
    }
}
type Field = (&'static str, &'static str, f32, f32);
fn fields(k: usize, i: usize) -> &'static [Field] {
    const LEVEL: Field = ("LEVEL", "level", 0., 1.);
    const TUNE: Field = ("TUNE", "tune", -6., 6.);
    const DECAY: Field = ("DECAY", "decay", 0.3, 2.);
    const TONE: Field = ("TONE", "tone", 0., 1.);
    const SNAPPY: Field = ("SNAPPY", "snappy", 0., 1.);
    const ATTACK: Field = ("ATTACK", "attack", 0., 1.);
    if k == 0 {
        match i {
            1 | 9 => &[LEVEL, TONE, DECAY],
            2 => &[LEVEL, ("TONE", "tune", -6., 6.), SNAPPY],
            3..=5 => &[LEVEL, TUNE],
            10 => &[LEVEL, DECAY],
            _ => &[LEVEL],
        }
    } else {
        match i {
            0 | 6 | 7 => &[LEVEL],
            1 => &[LEVEL, TUNE, ATTACK, DECAY],
            2 => &[LEVEL, TUNE, TONE, SNAPPY],
            3..=5 => &[LEVEL, TUNE, DECAY],
            8 | 9 => &[LEVEL, DECAY],
            _ => &[LEVEL, TUNE],
        }
    }
}

pub fn drums(app: &mut NativeApp, ui: &mut egui::Ui, r: Rect, k: usize) {
    skin::surface(
        ui,
        r,
        if k == 0 {
            Surface::Drum
        } else {
            Surface::Drum909
        },
    );
    let ids = if k == 0 { DRUM_IDS } else { DRUM_IDS_B };
    let names = [
        "",
        "BASS DRUM",
        "SNARE DRUM",
        "LO TOM",
        "MID TOM",
        "HI TOM",
        "RS",
        "HC",
        "CH",
        "OH",
        "CC",
        "RC",
    ];
    let positions = [
        0., 24., 66., 108., 150., 192., 234., 255., 276., 297., 318., 339.,
    ];
    let widths = [24., 42., 42., 42., 42., 42., 21., 21., 21., 21., 21., 23.];
    for i in 0..12 {
        let x = if k == 0 {
            7. + 22. * i as f32
        } else {
            positions[i]
        };
        let w = if k == 0 { 22. } else { widths[i] };
        if k == 1 && i < 11 {
            ui.painter().line_segment(
                [r.min + vec2(x + w, 0.), r.min + vec2(x + w, 87.)],
                Stroke::new(0.6, Color32::from_rgb(133, 141, 128)),
            );
        }
        let inst = if k == 0 {
            rect(r, x + 1., 74., 19., 8.)
        } else {
            rect(r, x + 1., 4., w - 2., 10.)
        };
        let selected = app.c.voices[k] == i;
        if k == 0 {
            ui.painter()
                .rect_filled(inst, 1., Color32::from_rgb(230, 225, 189));
            skin::text(
                ui,
                inst.center(),
                ids[i],
                7.,
                true,
                Color32::from_rgb(50, 53, 38),
                Align2::CENTER_CENTER,
            );
        } else {
            gradient(
                ui,
                inst,
                Color32::from_rgb(82, 87, 76),
                Color32::from_rgb(48, 58, 48),
            );
            ui.painter().rect_stroke(
                inst,
                0.,
                Stroke::new(0.6, Color32::from_rgb(127, 137, 122)),
                StrokeKind::Inside,
            );
            skin::text(
                ui,
                inst.center(),
                ids[i],
                7.,
                false,
                Color32::from_rgb(223, 141, 66),
                Align2::CENTER_CENTER,
            );
        }
        if selected {
            ui.painter().rect_stroke(
                inst,
                0.,
                Stroke::new(
                    1.,
                    if k == 0 {
                        Color32::from_rgb(187, 174, 107)
                    } else {
                        Color32::from_rgb(149, 155, 122)
                    },
                ),
                StrokeKind::Inside,
            );
        }
        if click(ui, inst, format!("voice:{k}:{i}")) {
            app.c.track = k + 2;
            app.c.action(&format!("voice:{k}:{i}"));
        }
        for (j, (title, key, min, max)) in fields(k, i).iter().enumerate() {
            if k == 1 && i == 8 && *key == "level" {
                continue;
            }
            let (dx, top) = if k == 0 {
                (
                    0.,
                    if i == 10 && j == 1 {
                        48.
                    } else {
                        [1., 24., 48.][j.min(2)]
                    },
                )
            } else {
                let dx = if (1..=5).contains(&i)
                    && (*key == "level"
                        || (i == 1 && *key == "decay")
                        || (i == 2 && *key == "snappy"))
                {
                    21.
                } else {
                    0.
                };
                let top = if ["attack", "decay", "tone", "snappy"].contains(key)
                    || (i >= 10 && *key == "tune")
                {
                    45.
                } else {
                    19.
                };
                (dx, top)
            };
            let colw = if k == 1 && i == 0 {
                24.
            } else {
                if k == 0 {
                    22.
                } else {
                    21.
                }
            };
            if !(k == 1 && i == 0) {
                label(
                    ui,
                    rect(r, x + dx, top, colw, 7.),
                    title,
                    6.,
                    if k == 0 { LIGHT } else { INK },
                );
            }
            let size = if k == 0 { 12. } else { 14. };
            let kr = rect(r, x + dx + (colw - size) / 2., top + 8., size, size);
            let kind = if k == 1 {
                KnobKind::Drum909
            } else if *key == "level" {
                KnobKind::Red
            } else {
                KnobKind::Cream
            };
            app.knob(
                ui,
                kr,
                title,
                &format!("drum:{k}:{i}:{key}"),
                (*min, *max),
                kind,
            );
        }
        if k == 0 && (3..=7).contains(&i) {
            let on = app.display.drums[k].voices[i].variant;
            let names = ["", "", "", "LC", "MC", "HC", "CL", "MA"];
            let variant_label = rect(r, x + 3., 48., 17., 8.);
            ui.painter()
                .rect_filled(variant_label, 1., Color32::from_rgb(227, 221, 187));
            label(
                ui,
                variant_label,
                names[i],
                7.,
                Color32::from_rgb(51, 56, 42),
            );
            let hit = rect(r, x + 8., 58., 7., 12.);
            if switch(ui, hit, format!("variant:{k}:{i}"), on) {
                app.c.track = k + 2;
                app.c.action(&format!("variant:{k}:{i}"));
            }
        }
        if k == 1 && i > 0 {
            let name = rect(r, x + 1., 75., w - 2., 7.);
            ui.painter().rect_stroke(
                name,
                1.,
                Stroke::new(0.5, Color32::from_rgb(138, 147, 130)),
                StrokeKind::Inside,
            );
            label(ui, name, names[i], 5., INK);
        }
    }
    if k == 1 {
        label(ui, rect(r, 1., 54., 23., 8.), "FLAM", 7., INK);
        app.knob(
            ui,
            rect(r, 5., 62., 15., 15.),
            "FLAM",
            "kit:1:flam",
            (0.005, 0.1),
            KnobKind::Drum909,
        );
        if switch(
            ui,
            rect(r, 7., 88., 12., 15.),
            "flam:1".to_owned(),
            app.c.flam,
        ) {
            app.c.track = k + 2;
            app.c.action("flam:1");
        }
    } else {
        let center = r.min + vec2(318., 56.);
        let dial = Rect::from_center_size(center, vec2(30., 31.));
        ui.painter()
            .circle_filled(center, 15., Color32::from_rgb(40, 47, 40));
        ui.painter()
            .circle_stroke(center, 15., Stroke::new(1., Color32::from_rgb(15, 25, 17)));
        let angle =
            ((225. + app.c.voices[0] as f32 * 30.).to_radians()) - std::f32::consts::FRAC_PI_2;
        ui.painter().line_segment(
            [center, center + vec2(angle.cos(), angle.sin()) * 10.],
            Stroke::new(2., Color32::from_rgb(120, 77, 51)),
        );
        let response = ui.interact(dial, ui.make_persistent_id("808-selector"), Sense::drag());
        if response.dragged() {
            let d = response.drag_motion().y;
            if d.abs() > 1. {
                let v = (app.c.voices[0] as i32 + if d < 0. { 1 } else { -1 }).clamp(0, 11);
                app.c.track = k + 2;
                app.c.action(&format!("voice:0:{v}"));
            }
        }
        for (i, id) in ids.iter().enumerate() {
            let a = ((225. + i as f32 * 30.).to_radians()) - std::f32::consts::FRAC_PI_2;
            let pos = r.min + vec2(318. + 22.8 * a.cos(), 54.4 + 24.6 * a.sin());
            let box_ = Rect::from_center_size(pos, vec2(12., 7.));
            if skin::button(ui, box_, &format!("dial:{i}"), id, app.c.voices[0] == i, 5.).clicked()
            {
                app.c.track = k + 2;
                app.c.action(&format!("voice:0:{i}"));
            }
        }
    }
    let voice = app.c.voices[k];
    let slot = app.c.song.slots[k + 2];
    let (left, top, width, gap, height) = if k == 0 {
        (13., 90., 254., 2., 20.)
    } else {
        (26., 84., 331., 4., 19.)
    };
    let step_width = (width - gap * 15.) / 16.;
    let colors = [
        (
            Color32::from_rgb(238, 85, 66),
            Color32::from_rgb(156, 36, 32),
        ),
        (
            Color32::from_rgb(255, 176, 61),
            Color32::from_rgb(170, 100, 18),
        ),
        (
            Color32::from_rgb(255, 240, 106),
            Color32::from_rgb(160, 165, 34),
        ),
        (
            Color32::from_rgb(255, 254, 243),
            Color32::from_rgb(174, 188, 172),
        ),
    ];
    for i in 0..16 {
        let key = rect(
            r,
            left + i as f32 * (step_width + gap),
            top,
            step_width,
            height,
        );
        let v = app.c.song.patterns[slot].drums[k][voice][i];
        let (top_color, bottom_color) = if k == 0 {
            colors[i / 4]
        } else {
            (
                Color32::from_rgb(245, 247, 237),
                Color32::from_rgb(138, 157, 136),
            )
        };
        gradient(ui, key, top_color, bottom_color);
        ui.painter().rect_stroke(
            key,
            1.,
            Stroke::new(0.8, Color32::from_rgb(42, 58, 42)),
            StrokeKind::Inside,
        );
        let light = Rect::from_min_size(
            pos2(
                key.center().x - if k == 0 { 3. } else { 2. },
                key.top() + 2.,
            ),
            vec2(if k == 0 { 6. } else { 5. }, if k == 0 { 3. } else { 2. }),
        );
        ui.painter().rect_filled(
            light,
            1.,
            if v > 0 {
                Color32::from_rgb(252, 53, 43)
            } else {
                Color32::from_rgb(108, 39, 33)
            },
        );
        if app.c.playing && app.c.position % 16 == i {
            ui.painter().line_segment(
                [
                    key.left_bottom() + vec2(1., -1.),
                    key.right_bottom() + vec2(-1., -1.),
                ],
                Stroke::new(2., Color32::from_rgb(252, 98, 54)),
            );
        }
        if click(ui, key, format!("drum:{k}:{voice}:{i}")) {
            app.c.track = k + 2;
            app.c.action(&format!("drum:{k}:{voice}:{i}"));
        }
        if k == 1 {
            let nr = rect(
                r,
                left + i as f32 * (step_width + gap),
                top + 20.,
                step_width,
                6.,
            );
            ui.painter()
                .rect_filled(nr, 0., Color32::from_rgb(56, 71, 53));
            label(ui, nr, &format!("{}", i + 1), 6., LIGHT);
        }
    }
}
