use crate::{audio::AudioBackend, controller::Controller, midi_device, platform};
use eframe::egui::{self, *};
use rb_model::Song;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

mod instruments;
mod rack;
mod skin;

const SIZE: Vec2 = vec2(624.0, 527.0);

pub struct NativeApp {
    c: Controller,
    display: Song,
    modal: bool,
    library: Vec<PathBuf>,
    selected_save: Option<PathBuf>,
    inputs: Vec<midi_device::MidiDevice>,
    midi: Option<midi_device::MidiInput>,
    midi_id: Option<u32>,
    midi_sync_flag: Arc<AtomicBool>,
    midi_tx: mpsc::Sender<Vec<u8>>,
    midi_rx: mpsc::Receiver<Vec<u8>>,
    muted: bool,
    confirm_delete: bool,
    confirm_new: bool,
}

impl NativeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, audio: AudioBackend) -> Self {
        Self::with_context(&cc.egui_ctx, audio)
    }

    fn with_context(ctx: &Context, audio: AudioBackend) -> Self {
        skin::install_fonts(ctx);
        let mut style = (*ctx.style()).clone();
        style.visuals = Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(101, 123, 125);
        style.visuals.window_fill = Color32::from_rgb(67, 89, 94);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(216, 222, 220);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(238, 190, 113);
        style.visuals.widgets.inactive.fg_stroke.color = skin::INK;
        style.visuals.widgets.hovered.fg_stroke.color = skin::INK;
        style.visuals.widgets.active.fg_stroke.color = skin::INK;
        style.visuals.widgets.noninteractive.fg_stroke.color = skin::LIGHT;
        style.visuals.selection.bg_fill = Color32::from_rgb(247, 170, 113);
        style.spacing.button_padding = vec2(4.0, 3.0);
        style.spacing.item_spacing = vec2(4.0, 5.0);
        style.spacing.interact_size = vec2(20.0, 14.0);
        style.spacing.slider_width = 80.0;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(7.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(7.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(6.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(12.0));
        style
            .text_styles
            .insert(TextStyle::Monospace, FontId::monospace(7.0));
        ctx.set_style(style);
        let (midi_tx, midi_rx) = mpsc::channel();
        Self {
            c: Controller::new(audio),
            display: Song::default(),
            modal: false,
            library: Vec::new(),
            selected_save: None,
            inputs: midi_device::devices().unwrap_or_default(),
            midi: None,
            midi_id: None,
            midi_sync_flag: Arc::new(AtomicBool::new(false)),
            midi_tx,
            midi_rx,
            muted: false,
            confirm_delete: false,
            confirm_new: false,
        }
    }

    fn view(&self) -> Song {
        let mut view = self.c.song.clone();
        if let Some(live) = self.c.live_controls.as_ref().filter(|_| self.c.playing) {
            view.synth = live.synth.clone();
            view.drums = live.drums.clone();
            view.channels = live.channels.clone();
            view.fx = live.fx.clone();
            view.enabled = live.enabled;
        }
        view
    }

    fn value(&self, key: &str) -> f32 {
        control(&self.display, key)
    }

    fn focus_track(&mut self, key: &str) {
        let parts: Vec<_> = key.split(':').collect();
        if let Some(index) = parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
            match parts[0] {
                "synth" => self.c.track = index.min(1),
                "drum" | "kit" => self.c.track = index.min(1) + 2,
                "channel" => self.c.track = index.min(3),
                _ => {}
            }
        }
    }

    fn knob(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        label: &str,
        key: &str,
        limits: (f32, f32),
        kind: skin::KnobKind,
    ) {
        let old = self.value(key);
        let mut response = ui.interact(r.expand(2.0), ui.id().with(key), Sense::click_and_drag());
        if response.clicked() || response.drag_started() {
            response.request_focus();
            self.focus_track(key);
        }
        let span = limits.1 - limits.0;
        let mut value = old;
        if response.dragged() {
            value -= response.drag_motion().y * ui.ctx().zoom_factor() * span / 150.0;
        }
        if response.hovered() {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            if delta != 0.0 {
                value += delta.signum() * span / 100.0;
            }
        }
        if response.has_focus() {
            if ui.input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowRight)) {
                value += span / 100.0;
            }
            if ui.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowLeft)) {
                value -= span / 100.0;
            }
        }
        value = value.clamp(limits.0, limits.1);
        if value != old {
            set_control(&mut self.c.song, key, value);
            self.c.changed(key, value);
            response.mark_changed();
        }
        response.widget_info(|| WidgetInfo::slider(true, value as f64, label));
        skin::knob(ui, r, (value - limits.0) / span, kind);
        if response.has_focus() {
            ui.painter().rect_stroke(
                r.expand(1.),
                0.,
                Stroke::new(0.5, Color32::from_rgb(255, 189, 79)),
                StrokeKind::Outside,
            );
        }
        response.on_hover_text(format!("{label}: {value:.3}"));
    }

    fn fader(&mut self, ui: &mut Ui, r: Rect, key: &str, limits: (f32, f32)) {
        let old = self.value(key);
        let mut response = ui.interact(r.expand(2.0), ui.id().with(key), Sense::click_and_drag());
        if response.clicked() || response.drag_started() {
            response.request_focus();
            self.focus_track(key);
        }
        let span = limits.1 - limits.0;
        let mut value = old;
        if response.dragged() || response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                value = limits.0 + (1.0 - (pointer.y - r.top()) / r.height()) * span;
            }
        }
        if response.has_focus() {
            if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                value += span / 100.0;
            }
            if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                value -= span / 100.0;
            }
        }
        value = value.clamp(limits.0, limits.1);
        if value != old {
            set_control(&mut self.c.song, key, value);
            self.c.changed(key, value);
            response.mark_changed();
        }
        response.widget_info(|| WidgetInfo::slider(true, value as f64, key));
        skin::fader(ui, r, (value - limits.0) / span);
        response.on_hover_text(format!("{key}: {value:.3}"));
    }

    fn file_result(&mut self, result: Result<Option<PathBuf>, String>, operation: &str) {
        let result = match result {
            Ok(Some(path)) => match operation {
                "open" => self.c.load_path(&path),
                "wav" => self.c.export_wav(&path),
                _ => self.c.save_path(&path),
            },
            Ok(None) => return,
            Err(e) => Err(e),
        };
        self.c.status = match result {
            Ok(()) => format!("{} complete.", operation),
            Err(e) => e,
        };
    }

    fn library(&mut self, ctx: &Context) {
        let response = egui::Modal::new(Id::new("song-file-modal")).show(ctx, |ui| {
            let zoom = ctx.zoom_factor();
            ui.set_width(
                (720.0 / zoom)
                    .min(ctx.content_rect().width() - 32.0 / zoom)
                    .max(1.0),
            );
            let style = ui.style_mut();
            style
                .text_styles
                .insert(TextStyle::Body, FontId::proportional(14.0 / zoom));
            style
                .text_styles
                .insert(TextStyle::Button, FontId::proportional(14.0 / zoom));
            style
                .text_styles
                .insert(TextStyle::Heading, FontId::proportional(18.0 / zoom));
            style.spacing.interact_size = vec2(24.0 / zoom, 26.0 / zoom);
            style.spacing.item_spacing = vec2(8.0 / zoom, 8.0 / zoom);
            style.spacing.button_padding = vec2(6.0 / zoom, 4.0 / zoom);
            style.spacing.slider_width = 100.0 / zoom;
            ui.heading("SONG / FILE · LIBRARY");
            ui.horizontal_wrapped(|ui| {
                ui.label("Song");
                ui.add(TextEdit::singleline(&mut self.c.song.name).desired_width(240.0 / zoom));
                if ui.button("NEW").clicked() {
                    self.confirm_new = true;
                }
                if ui.button("IMPORT RBS / JSON").clicked() {
                    let result = platform::open_song_dialog();
                    self.file_result(result, "open");
                }
                if ui.button("SAVE JSON AS…").clicked() {
                    let result = platform::save_song_dialog(&format!("{}.json", self.c.song.name));
                    self.file_result(result, "save");
                }
            });
            if self.confirm_new {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Replace current song? Save it first if needed.");
                    if ui.button("REPLACE").clicked() {
                        self.c.install_song(Song::default());
                        self.confirm_new = false;
                    }
                    if ui.button("CANCEL").clicked() {
                        self.confirm_new = false;
                    }
                });
            }
            ui.separator();
            ui.label("Named song library · %LOCALAPPDATA%\\RB338Craft\\Songs");
            ui.horizontal_wrapped(|ui| {
                if ui.button("SAVE TO LIBRARY").clicked() {
                    let result = platform::library_path(&self.c.song.name)
                        .and_then(|path| self.c.save_path(&path));
                    self.c.status = result.map(|_| "Song saved.".into()).unwrap_or_else(|e| e);
                    self.library = platform::library_songs().unwrap_or_default();
                }
                ComboBox::from_id_salt("native-saves")
                    .width(260.0 / zoom)
                    .selected_text(
                        self.selected_save
                            .as_ref()
                            .and_then(|p| p.file_stem())
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Select a saved song".into()),
                    )
                    .show_ui(ui, |ui| {
                        for path in &self.library {
                            ui.selectable_value(
                                &mut self.selected_save,
                                Some(path.clone()),
                                path.file_stem().unwrap_or_default().to_string_lossy(),
                            );
                        }
                    });
                if ui.button("LOAD").clicked() {
                    if let Some(path) = self.selected_save.clone() {
                        self.c.status = self
                            .c
                            .load_path(&path)
                            .map(|_| "Song loaded.".into())
                            .unwrap_or_else(|e| e);
                    }
                }
                if ui.button("DELETE").clicked() {
                    self.confirm_delete = true;
                }
            });
            if self.confirm_delete {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Delete selected saved file?");
                    if ui.button("DELETE FILE").clicked() {
                        if let Some(path) = self.selected_save.take() {
                            self.c.status = std::fs::remove_file(path)
                                .map(|_| "Deleted.".into())
                                .unwrap_or_else(|e| e.to_string());
                            self.library = platform::library_songs().unwrap_or_default();
                        }
                        self.confirm_delete = false;
                    }
                    if ui.button("CANCEL").clicked() {
                        self.confirm_delete = false;
                    }
                });
            }
            ui.separator();
            for t in 0..4 {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("TRACK {}", t + 1));
                    for (label, action) in [
                        ("COPY", "copy"),
                        ("PASTE", "paste"),
                        ("RANDOM", "random"),
                        ("CLEAR", "clear"),
                    ] {
                        if ui.button(label).clicked() {
                            self.c.action(&format!("{action}:{t}"));
                        }
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                for (label, action) in [
                    ("ADD SCENE", "scene_add"),
                    ("REMOVE LAST", "scene_remove"),
                    ("CLEAR SCENES", "scene_clear"),
                    ("COPY LOOP", "measure_copy"),
                    ("CUT LOOP", "measure_cut"),
                    ("INSERT LOOP", "measure_insert"),
                    ("REPLACE LOOP", "measure_replace"),
                ] {
                    if ui.button(label).clicked() {
                        self.c.action(action);
                    }
                }
            });
            ScrollArea::vertical()
                .max_height(100.0 / zoom)
                .id_salt("scene-list")
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for i in 0..self.c.song.arrangement.len() {
                            if ui
                                .selectable_label(self.c.position / 16 == i, (i + 1).to_string())
                                .clicked()
                            {
                                self.c.action(&format!("scene:{i}"));
                            }
                        }
                    });
                });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(self.c.song.vintage, "VINTAGE")
                    .clicked()
                {
                    self.c.action("vintage");
                }
                if ui.selectable_label(self.c.metronome, "METRONOME").clicked() {
                    self.c.action("metronome");
                }
                if ui.checkbox(&mut self.muted, "MUTE OUTPUT").changed() {
                    self.c
                        .audio
                        .send(crate::audio::AudioMessage::Mute(self.muted));
                }
                let mut wet = self.c.song.fx.delay_wet;
                if ui
                    .add(Slider::new(&mut wet, 0.0..=1.0).text("Delay wet"))
                    .changed()
                {
                    self.c.song.fx.delay_wet = wet;
                    self.c.changed("fx:delay_wet", wet);
                }
                if ui.button("EXPORT LOOP WAV…").clicked() {
                    let result = platform::save_wav_dialog(&format!("{}.wav", self.c.song.name));
                    self.file_result(result, "wav");
                }
            });
            ui.separator();
            ui.label("Native MIDI input");
            ui.horizontal_wrapped(|ui| {
                if ui.button("REFRESH INPUTS").clicked() {
                    match midi_device::devices() {
                        Ok(inputs) => self.inputs = inputs,
                        Err(e) => self.c.status = e,
                    }
                }
                let old = self.midi_id;
                ComboBox::from_id_salt("midi-device")
                    .width(240.0 / zoom)
                    .selected_text(
                        self.inputs
                            .iter()
                            .find(|d| Some(d.id) == self.midi_id)
                            .map(|d| d.name.as_str())
                            .unwrap_or("MIDI OFF"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.midi_id, None, "MIDI OFF");
                        for input in &self.inputs {
                            ui.selectable_value(&mut self.midi_id, Some(input.id), &input.name);
                        }
                    });
                if old != self.midi_id {
                    self.midi = None;
                    if let Some(id) = self.midi_id {
                        match midi_device::open(
                            id,
                            self.midi_tx.clone(),
                            self.c.audio.sender(),
                            self.midi_sync_flag.clone(),
                        ) {
                            Ok(input) => self.midi = Some(input),
                            Err(e) => {
                                self.c.status = e;
                                self.midi_id = None;
                            }
                        }
                    }
                }
                let mut channel = self.c.midi_channel + 1;
                ui.label("Channel");
                if ui.add(DragValue::new(&mut channel).range(1..=16)).changed() {
                    self.c.midi_channel = channel - 1;
                }
                if ui.selectable_label(self.c.midi_sync, "MIDI SYNC").clicked() {
                    self.c.action("midi_sync");
                }
            });
            ui.label(&self.c.status);
            if ui.button("CLOSE").clicked() {
                self.modal = false;
            }
        });
        if response.should_close() {
            self.modal = false;
        }
    }
}

impl NativeApp {
    fn draw_frame(&mut self, ctx: &Context) {
        self.c.poll();
        while let Ok(bytes) = self.midi_rx.try_recv() {
            self.c.midi(&bytes);
        }
        self.midi_sync_flag
            .store(self.c.midi_sync, Ordering::Relaxed);
        self.display = self.view();
        let old_zoom = ctx.zoom_factor();
        let available = ctx.content_rect().size() * old_zoom;
        let zoom = fit_scale(available);
        if (zoom - old_zoom).abs() > 0.005 {
            ctx.set_zoom_factor(zoom);
            ctx.request_repaint();
        }
        if !self.modal && !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(Key::Space)) {
                self.c.action("play");
            }
            for (n, key) in [
                Key::A,
                Key::W,
                Key::S,
                Key::E,
                Key::D,
                Key::F,
                Key::T,
                Key::G,
                Key::Y,
                Key::H,
                Key::U,
                Key::J,
            ]
            .iter()
            .enumerate()
            {
                if ctx.input(|i| i.key_pressed(*key)) {
                    self.c
                        .action(&format!("pitchkey:{}:{n}", self.c.track.min(1)));
                }
            }
        }
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_gray(170)))
            .show(ctx, |ui| {
                let canvas = Rect::from_min_size(
                    pos2((ui.max_rect().width() - SIZE.x).max(0.0) / 2.0, 0.0),
                    SIZE,
                );
                rack::draw(self, ui, canvas);
            });
        if self.modal {
            self.library(ctx);
        }
        self.midi_sync_flag
            .store(self.c.midi_sync, Ordering::Relaxed);
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

impl eframe::App for NativeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.draw_frame(ctx);
    }
}

pub(crate) fn check_renderer(audio: AudioBackend) -> Result<(), String> {
    let ctx = Context::default();
    let mut app = NativeApp::with_context(&ctx, audio);
    for size in [vec2(1280.0, 1080.0), vec2(800.0, 600.0)] {
        for _ in 0..3 {
            let logical = size / ctx.zoom_factor();
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, logical)),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| app.draw_frame(ctx));
            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
            if primitives.is_empty() {
                return Err("Rack renderer emitted no drawing primitives".into());
            }
            for primitive in primitives {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    if !mesh.is_valid()
                        || mesh
                            .vertices
                            .iter()
                            .any(|vertex| !vertex.pos.x.is_finite() || !vertex.pos.y.is_finite())
                    {
                        return Err("Rack renderer emitted an invalid mesh".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn fit_scale(available: Vec2) -> f32 {
    (available.x / SIZE.x).min(available.y / SIZE.y).max(0.1)
}
fn rect(parent: Rect, x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(parent.min + vec2(x, y), vec2(w, h))
}
fn control(song: &Song, key: &str) -> f32 {
    let p: Vec<_> = key.split(':').collect();
    let n = |i: usize| p.get(i).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    match p.as_slice() {
        ["shuffle"] => song.shuffle,
        ["synth", _, f] => {
            let s = &song.synth[n(1)];
            match *f {
                "tune" => s.tune,
                "cutoff" => s.cutoff,
                "resonance" => s.resonance,
                "envelope" => s.envelope,
                "decay" => s.decay,
                "accent" => s.accent,
                _ => 0.,
            }
        }
        ["drum", _, _, f] => {
            let d = &song.drums[n(1)].voices[n(2)];
            match *f {
                "level" => d.level,
                "tune" => d.tune,
                "decay" => d.decay,
                "tone" => d.tone,
                "snappy" => d.snappy,
                "attack" => d.attack,
                _ => 0.,
            }
        }
        ["kit", _, "flam"] => song.drums[n(1)].flam,
        ["channel", _, f] => {
            let c = &song.channels[n(1)];
            match *f {
                "level" => c.level,
                "pan" => c.pan,
                "delay" => c.delay,
                _ => 0.,
            }
        }
        ["fx", f] => {
            let fxs = &song.fx;
            match *f {
                "master" => fxs.master,
                "delay_pan" => fxs.delay_pan,
                "delay_feedback" => fxs.delay_feedback,
                "delay_wet" => fxs.delay_wet,
                "distortion" => fxs.distortion,
                "distortion_shape" => fxs.distortion_shape,
                "filter_cutoff" => fxs.filter_cutoff,
                "filter_q" => fxs.filter_q,
                "filter_amount" => fxs.filter_amount,
                "filter_decay" => fxs.filter_decay,
                "compressor_ratio" => fxs.compressor_ratio,
                "compressor_threshold" => fxs.compressor_threshold,
                _ => 0.,
            }
        }
        _ => 0.,
    }
}
fn set_control(song: &mut Song, key: &str, v: f32) {
    let p: Vec<_> = key.split(':').collect();
    let n = |i: usize| p.get(i).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    match p.as_slice() {
        ["shuffle"] => song.shuffle = v,
        ["synth", _, f] => {
            let s = &mut song.synth[n(1)];
            match *f {
                "tune" => s.tune = v,
                "cutoff" => s.cutoff = v,
                "resonance" => s.resonance = v,
                "envelope" => s.envelope = v,
                "decay" => s.decay = v,
                "accent" => s.accent = v,
                _ => {}
            }
        }
        ["drum", _, _, f] => {
            let d = &mut song.drums[n(1)].voices[n(2)];
            match *f {
                "level" => d.level = v,
                "tune" => d.tune = v,
                "decay" => d.decay = v,
                "tone" => d.tone = v,
                "snappy" => d.snappy = v,
                "attack" => d.attack = v,
                _ => {}
            }
        }
        ["kit", _, "flam"] => song.drums[n(1)].flam = v,
        ["channel", _, f] => {
            let c = &mut song.channels[n(1)];
            match *f {
                "level" => c.level = v,
                "pan" => c.pan = v,
                "delay" => c.delay = v,
                _ => {}
            }
        }
        ["fx", f] => {
            let fx = &mut song.fx;
            match *f {
                "master" => fx.master = v,
                "delay_pan" => fx.delay_pan = v,
                "delay_feedback" => fx.delay_feedback = v,
                "delay_wet" => fx.delay_wet = v,
                "distortion" => fx.distortion = v,
                "distortion_shape" => fx.distortion_shape = v,
                "filter_cutoff" => fx.filter_cutoff = v,
                "filter_q" => fx.filter_q = v,
                "filter_amount" => fx.filter_amount = v,
                "filter_decay" => fx.filter_decay = v,
                "compressor_ratio" => fx.compressor_ratio = v,
                "compressor_threshold" => fx.compressor_threshold = v,
                _ => {}
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rack_fits_large_and_small_windows() {
        for size in [
            vec2(2560., 1285.),
            vec2(1920., 1080.),
            vec2(1280., 720.),
            vec2(800., 600.),
        ] {
            let scale = fit_scale(size);
            assert!(SIZE.x * scale <= size.x + 0.01);
            assert!(SIZE.y * scale <= size.y + 0.01);
        }
    }
    #[test]
    fn displayed_controls_round_trip() {
        let mut song = Song::default();
        for (key, value) in [
            ("synth:1:cutoff", 2400.),
            ("drum:1:2:snappy", 0.8),
            ("channel:3:pan", -0.4),
            ("fx:delay_wet", 0.3),
            ("kit:1:flam", 0.05),
            ("shuffle", 20.),
        ] {
            set_control(&mut song, key, value);
            assert_eq!(control(&song, key), value);
        }
        song.validate().unwrap();
    }
}
