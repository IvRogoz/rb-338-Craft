mod export;
mod midi;
use rb_model::{ControlState, Pattern, Song, DRUM_IDS, DRUM_IDS_B};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{Document, Element, Event, HtmlInputElement, HtmlSelectElement};

#[wasm_bindgen(raw_module = "/audio.js")]
extern "C" {
    #[wasm_bindgen(js_name=startAudio, catch)]
    fn start_audio(song: &str) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(js_name=setSong)]
    fn set_song(song: &str);
    fn command(cmd: u32, value: f64);
    #[wasm_bindgen(js_name=recordCommand)]
    fn record_command(value: f64, request_id: u32);
}
#[wasm_bindgen(raw_module = "/midi.js")]
extern "C" {
    #[wasm_bindgen(js_name=requestMidi,catch)]
    fn request_midi() -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(js_name=selectMidiInput)]
    fn select_midi_input(id: &str);
    #[wasm_bindgen(js_name=closeMidi)]
    fn close_midi();
}
struct App {
    song: Song,
    live_controls: Option<ControlState>,
    track: usize,
    voice: usize,
    clipboard: Option<(usize, Pattern)>,
    position: usize,
    playing: bool,
    metronome: bool,
    bank: [usize; 4],
    edit: [usize; 2],
    pitch_mode: [bool; 2],
    voices: [usize; 2],
    flam: bool,
    recording: bool,
    tick: usize,
    measure_clipboard: Option<rb_model::arrangement::MeasureClipboard>,
    punch: Option<Punch>,
    exporting: bool,
    audio_ready: bool,
    pending_ack: Option<u32>,
    next_request: u32,
    pending_song: Option<Song>,
    midi_enabled: bool,
    midi_sync: bool,
    midi_channel: u8,
    midi_program: bool,
    midi_device: String,
    midi_inputs: Vec<(String, String)>,
    midi_clock: midi::MidiClock,
}
fn doc() -> Document {
    web_sys::window().unwrap().document().unwrap()
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn btn(action: &str, label: &str) -> String {
    format!("<button data-action='{action}'>{label}</button>")
}
fn digits(value: usize, width: usize) -> String {
    format!("{value:0width$}").chars().map(|c| {
        format!("<span class='digit d{c}'>{c}<i></i><i></i><i></i><i></i><i></i><i></i><i></i></span>")
    }).collect()
}
fn sync(a: &App) {
    if let Ok(s) = serde_json::to_string(&a.song) {
        set_song(&s);
    }
}
fn status(s: &str) {
    if let Some(e) = doc().get_element_by_id("status") {
        e.set_text_content(Some(s));
    }
    if doc().get_element_by_id("song-dialog").is_some_and(|d| d.has_attribute("open")) {
        if let Some(e) = doc().get_element_by_id("song-status") {
            e.set_text_content(Some(s));
        }
    }
}
fn knob(label: &str, key: &str, v: f32, min: f32, max: f32, step: f32, color: &str) -> String {
    let angle = -135.0 + 270.0 * (v - min) / (max - min);
    format!("<label class='knobbox' data-control='{key}' title='{label}'><span class='knob {color}' style='--angle:{angle}deg'><input aria-label='{label}' type=range data-key='{key}' value='{v}' min='{min}' max='{max}' step='{step}'></span><span>{label}</span></label>")
}
fn toggle(action: &str, label: &str, on: bool) -> String {
    format!(
        "<button data-action='{action}' class='switch {}'>{label}</button>",
        if on { "lit" } else { "" }
    )
}
fn pattern_box(a: &App, t: usize) -> String {
    let slot = a.song.slots[t];
    let p = &a.song.patterns[slot];
    let mut h = format!(
        "<div class=patbox><div class=pathead>{} PATTERN</div><div class=patgrid>",
        toggle(&format!("enable:{t}"), "●", a.song.enabled[t])
    );
    for i in 0..8 {
        h += &format!(
            "<button data-action='pattern:{t}:{i}' class='{}'>{}</button>",
            if slot % 8 == i && slot / 8 == a.bank[t] {
                "selected"
            } else {
                ""
            },
            i + 1
        )
    }
    h.push_str("</div><div class=patlabel>BANK</div><div class=bankrow>");
    for i in 0..4 {
        h += &format!(
            "<button data-action='bank:{t}:{i}' class='{}'>{}</button>",
            if a.bank[t] == i { "selected" } else { "" },
            ["A", "B", "C", "D"][i]
        )
    }
    h+=&format!("</div><div class='shuffle-label patlabel'>SHUFFLE {} &nbsp; STEPS</div><div class=length>{}<span class=led>{}</span>{}</div></div>",toggle(&format!("shuffle_track:{t}"),"●",a.song.shuffle_tracks[t]),btn(&format!("steps_down:{t}"),"−"),digits(p.lengths[t],2),btn(&format!("steps_up:{t}"),"+"));
    h
}
fn mixer(a: &App, t: usize) -> String {
    let c = &a.song.channels[t];
    format!("<div class=mixstrip><div class=mixhead>{} MIX</div>{}<label class='fader mixlevel'>LEVEL<input aria-label='Track {} level' type=range data-key='channel:{t}:level' min=0 max=1 step=.01 value='{}'></label>{}{}{}{}<div id='channel-meter-{t}' class=smallmeter></div></div>",toggle(&format!("mute:{t}"),"●",!c.mute),knob("PAN",&format!("channel:{t}:pan"),c.pan,-1.,1.,0.01,"black"),t+1,c.level,knob("DELAY",&format!("channel:{t}:delay"),c.delay,0.,1.,0.01,"black"),toggle(&format!("dist:{t}"),"DIST",c.distortion),toggle(&format!("filter:{t}"),"PCF",c.filter),toggle(&format!("comp:{t}"),"COMP",c.compressor))
}
fn pitch_with_key(note: u8, key: u8) -> u8 {
    let octave = (note / 12).clamp(2, 4);
    octave * 12 + key.min(12)
}
fn toggle_octave(note: u8, up: bool) -> u8 {
    let pitch = note % 12;
    let octave = (note / 12).clamp(2, 4);
    let desired = if up { 4 } else { 2 };
    (if octave == desired { 3 } else { desired }) * 12 + pitch
}
fn reset_editor(a: &mut App) {
    a.bank = a.song.slots.map(|slot| slot / 8);
    a.edit = [0, 0];
}
fn synth_panel(a: &App, t: usize) -> String {
    let s = &a.song.synth[t];
    let n = &a.song.patterns[a.song.slots[t]].notes[t][a.edit[t]];
    let mut h=format!("<div class='device synth'><div class=synthtop><div class=wave><span>WAVEFORM</span><button data-action='wave:{t}' data-wave='{}' aria-label='Synth {} waveform'></button></div>",esc(&s.wave),t+1);
    for (name, key, v, min, max, step) in [
        ("TUNE", "tune", s.tune, -12., 12., 1.),
        ("CUTOFF", "cutoff", s.cutoff, 100., 6000., 10.),
        ("RESO", "resonance", s.resonance, 0., 1., 0.01),
        ("ENV.MOD", "envelope", s.envelope, 0., 5000., 10.),
        ("DECAY", "decay", s.decay, 0.05, 0.8, 0.01),
        ("ACCENT", "accent", s.accent, 0., 1., 0.01),
    ] {
        h += &knob(
            name,
            &format!("synth:{t}:{key}"),
            v,
            min,
            max,
            step,
            "silver",
        )
    }
    h+=&format!("<div class=editstep>EDIT STEP<br><span class=led>{}</span></div></div><div class=synthbottom><div class=pitchmode>PITCH MODE{}{}{}</div><div class=pitchkeys>",digits(a.edit[t]+1,2),toggle(&format!("pitchmode:{t}"),if a.pitch_mode[t]{"ON"}else{"OFF"},a.pitch_mode[t]),btn(&format!("clear:{t}"),"CLEAR")," ");
    for i in 0..13 {
        let sharp = [1, 3, 6, 8, 10].contains(&i);
        h+=&format!("<button aria-label='Synth {} pitch {}' class='pitch {} {}' data-action='pitchkey:{t}:{i}'>{}</button>",t+1,i,if sharp{"sharp"}else{"natural"},if (n.note%12==i as u8 && i<12 && !(i==0 && n.note>=48)) || (i==12 && n.note%12==0 && n.note>=48) {"selected"}else{""},["C","C♯","D","D♯","E","F","F♯","G","G♯","A","A♯","B","C"][i]);
    }
    h.push_str("</div><div class=stepcontrols><span class=slide-display>𝄽<i></i></span>");
    for (key, label, on) in [
        ("down", "▼ DOWN", n.note < 36),
        ("up", "▲ UP", n.note >= 48),
        ("note", "NOTE / PAUSE", n.gate),
        ("accent", "ACCENT", n.accent),
        ("slide", "SLIDE", n.slide),
        ("back", "BACK", false),
        ("next", "STEP", false),
    ] {
        h += &toggle(&format!("{key}:{t}"), label, on)
    }
    h.push_str("</div></div></div>");
    h
}
fn drum_panel(a: &App, k: usize) -> String {
    let t = k + 2;
    let kit = &a.song.drums[k];
    let ids = if k == 0 { DRUM_IDS } else { DRUM_IDS_B };
    let mut h = format!("<div class='device drum kit{k}'><div class=drumcols>");
    for (i, id) in ids.iter().enumerate() {
        let d = &kit.voices[i];
        h += &format!(
            "<div class=drumcol data-voice='{i}'><button data-action='voice:{k}:{i}' class='inst {}'>{id}</button>",
            if a.voices[k] == i { "active" } else { "" }
        );
        let fields: &[(&str, &str, f32, f32, f32, f32)] = if k == 0 {
            match i {
                0 => &[("LEVEL", "level", d.level, 0., 1., 0.01)],
                1 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TONE", "tone", d.tone, 0., 1., 0.01),
                    ("DECAY", "decay", d.decay, 0.3, 2., 0.01),
                ],
                2 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TONE", "tune", d.tune, -6., 6., 0.1),
                    ("SNAPPY", "snappy", d.snappy, 0., 1., 0.01),
                ],
                3..=5 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TUNE", "tune", d.tune, -6., 6., 0.1),
                ],
                9 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TONE", "tone", d.tone, 0., 1., 0.01),
                    ("DECAY", "decay", d.decay, 0.3, 2., 0.01),
                ],
                10 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("DECAY", "decay", d.decay, 0.3, 2., 0.01),
                ],
                _ => &[("LEVEL", "level", d.level, 0., 1., 0.01)],
            }
        } else {
            match i {
                0 | 6 | 7 => &[("LEVEL", "level", d.level, 0., 1., 0.01)],
                1 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TUNE", "tune", d.tune, -6., 6., 0.1),
                    ("ATTACK", "attack", d.attack, 0., 1., 0.01),
                    ("DECAY", "decay", d.decay, 0.3, 2., 0.01),
                ],
                2 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TUNE", "tune", d.tune, -6., 6., 0.1),
                    ("TONE", "tone", d.tone, 0., 1., 0.01),
                    ("SNAP", "snappy", d.snappy, 0., 1., 0.01),
                ],
                3..=5 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TUNE", "tune", d.tune, -6., 6., 0.1),
                    ("DECAY", "decay", d.decay, 0.3, 2., 0.01),
                ],
                8 | 9 => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("DECAY", "decay", d.decay, 0.3, 2., 0.01),
                ],
                _ => &[
                    ("LEVEL", "level", d.level, 0., 1., 0.01),
                    ("TUNE", "tune", d.tune, -6., 6., 0.1),
                ],
            }
        };
        for (label, key, v, min, max, step) in fields {
            h += &knob(
                label,
                &format!("drum:{k}:{i}:{key}"),
                *v,
                *min,
                *max,
                *step,
                if k == 0 && *key == "level" {
                    "red"
                } else if k == 0 {
                    "cream"
                } else {
                    "silver"
                },
            )
        }
        if k == 0 && (3..=7).contains(&i) {
            h += &toggle(
                &format!("variant:{k}:{i}"),
                if d.variant {
                    ["", "", "", "CONGA", "CONGA", "CONGA", "CLAVES", "MARACAS"][i]
                } else {
                    ["", "", "", "TOM", "TOM", "TOM", "RIM", "CLAP"][i]
                },
                d.variant,
            )
        }
        if k == 1 {
            h += &format!("<span class='voice-name'>{}</span>", ["", "BASS DRUM", "SNARE DRUM", "LO TOM", "MID TOM", "HI TOM", "RS", "HC", "CH", "OH", "CC", "RC"][i]);
        }
        h.push_str("</div>");
    }
    h.push_str("</div><div class=drumbottom>");
    if k == 1 {
        h += &knob("FLAM", "kit:1:flam", kit.flam, 0.005, 0.1, 0.005, "silver");
        h += &toggle("flam:1", "FLAM", a.flam)
    } else {
        h += &format!("<div class=dial><label class='dial-knob'><span class='knob black' style='--angle:{}deg'><input type=range aria-label='808 instrument selector' data-dial='0' min=0 max=11 step=1 value='{}'></span><span>{}</span></label>",225+a.voices[0]*30,a.voices[0],DRUM_IDS[a.voices[0]]);
        for (i, id) in DRUM_IDS.iter().enumerate() {
            let angle = ((225 + i * 30) as f64).to_radians() - std::f64::consts::FRAC_PI_2;
            let x = 50. + 30. * angle.cos();
            let y = 48. + 30. * angle.sin();
            h+=&format!("<button class='dial-label' style='left:{x}%;top:{y}%' data-action='voice:0:{i}'>{id}</button>");
        }
        h.push_str("</div>");
    }
    h.push_str("<div class=drumsteps>");
    let voice = a.voices[k];
    for i in 0..16 {
        let v = a.song.patterns[a.song.slots[t]].drums[k][voice][i];
        h+=&format!("<button aria-label='{} step {}' data-action='drum:{k}:{voice}:{i}' data-track='{t}' data-step='{i}' class='step s{} {}'><i></i><span class='step-number'>{}</span> <span class='step-flag'>{}</span></button>",ids[voice],i+1,i/4,if v>0{"hit"}else{""},i+1,["","","A","F"][v as usize]);
    }
    h.push_str("</div></div></div>");
    h
}
fn master(a: &App) -> String {
    let f = &a.song.fx;
    let mut h=format!("<aside class=master><div class=fxblock><div class=gold>MASTER</div><div class=masterbody><div id=meter-0 class=meter></div><label class=fader>LEVEL<input type=range aria-label='Master level' data-key='fx:master' min=0 max=1.5 step=.01 value='{}'></label><div id=meter-1 class=meter></div>{}</div></div>",f.master,toggle("master_comp","COMP",f.compressor_master));
    h+=&format!("<div class=fxblock><div class=blue>PCF {}<span id=effect-meter-7 class=effectmeter aria-label='PCF input level'></span></div><div class=fxline>{}<span id=pcf-number class=led>{}</span>{}<button data-action=filter_mode data-mode='{}' aria-label='PCF mode'>{}</button></div><div class=fxknobs>",toggle("fx_toggle:filter_enabled","●",f.filter_enabled),btn("fx_step:filter_pattern:-1","−"),digits(f.filter_pattern,2),btn("fx_step:filter_pattern:1","+"),esc(&f.filter_mode),esc(&f.filter_mode));
    for (n, k, v, min, max, step) in [
        ("FREQ", "filter_cutoff", f.filter_cutoff, 100., 8000., 10.),
        ("Q", "filter_q", f.filter_q, 0.5, 15., 0.1),
        ("AMT", "filter_amount", f.filter_amount, 0., 1., 0.01),
        ("DEC", "filter_decay", f.filter_decay, 0.1, 10., 0.01),
    ] {
        h+=&format!("<label class=fader>{n}<input aria-label='{n}' type=range data-key='fx:{k}' value='{v}' min='{min}' max='{max}' step='{step}'></label>")
    }
    h.push_str("</div></div>");
    h+=&format!("<div class=fxblock><div class=blue>DELAY {}<span id=effect-meter-8 class=effectmeter aria-label='Delay input level'></span></div><div class=fxline>{}<span id=delay-number class=led>{}</span>{}{}</div><div class=fxknobs>",toggle("fx_toggle:delay_enabled","●",f.delay_enabled),btn("fx_step:delay_steps:-1","−"),digits(f.delay_steps,2),btn("fx_step:delay_steps:1","+"),toggle("fx_toggle:delay_triplet","TRIPLET",f.delay_triplet));
    for (n, k, v, min, max) in [
        ("PAN", "delay_pan", f.delay_pan, -1., 1.),
        ("F.BACK", "delay_feedback", f.delay_feedback, 0., 0.98),
    ] {
        h += &knob(n, &format!("fx:{k}"), v, min, max, 0.01, "black")
    }
    h.push_str("</div></div>");
    for (title, enabled, fields) in [
        (
            "DIST",
            "distortion_enabled",
            vec![
                ("AMOUNT", "distortion", f.distortion, 0., 1.),
                ("SHAPE", "distortion_shape", f.distortion_shape, 0., 1.),
            ],
        ),
        (
            "COMP",
            "compressor_enabled",
            vec![
                ("RATIO", "compressor_ratio", f.compressor_ratio, 1., 20.),
                (
                    "THRES",
                    "compressor_threshold",
                    f.compressor_threshold,
                    -40.,
                    0.,
                ),
            ],
        ),
    ] {
        h += &format!(
            "<div class=fxblock><div class=blue>{title} {}<span id=effect-meter-{} class=effectmeter aria-label='{title} input level'></span></div>{}<div class=fxknobs>",
            toggle(
                &format!("fx_toggle:{enabled}"),
                "●",
                if title == "DIST" {
                    f.distortion_enabled
                } else {
                    f.compressor_enabled
                }
            ),
            if title == "DIST" {9} else {10},
            if title == "COMP" {"<div id=reduction-meter class=reductionmeter aria-label='Compressor gain reduction'></div>"} else {""}
        );
        for (n, k, v, min, max) in fields {
            h += &knob(n, &format!("fx:{k}"), v, min, max, 0.01, "black")
        }
        h.push_str("</div></div>");
    }
    h.push_str("</aside>");
    h
}
fn transport(a: &App) -> String {
    let s = &a.song;
    let mut h = format!(
        "<header class=transport><div class=brand><span class=brand-lights><i></i><i></i><i></i><i></i></span><span class=brand-name>ReBirth</span><b>RB-338</b></div>{}<div class=sync-indicators>{}{}</div><div class=mode-selector>{}{}</div><div class=transportbuttons>{}{}{}{}{}</div>",
        knob("SHUFFLE", "shuffle", s.shuffle, 0., 60., 1., "black"),
        toggle("midi_sync", "Sync", a.midi_sync),
        toggle("midi_enable", "MIDI", a.midi_enabled),
        toggle("pattern_mode", "Pattern", !s.song_mode),
        toggle("song_mode", "Song mode", s.song_mode),
        format!("<button data-action='play' class='{}' aria-label='Play' aria-pressed='{}'>▶</button>",
            if a.playing { "lit" } else { "" }, a.playing),
        btn("stop", "■"), btn("transport:-10", "◀◀"), btn("transport:10", "▶▶"),
        toggle("record", "● REC", a.recording)
    );
    for (key, label, value) in [
        ("tempo", "Tempo", s.bpm.round() as usize),
        ("bar", "Bar", a.position / 16 + 1),
        ("start", "Start", s.start + 1),
        ("length", "Length", s.length),
    ] {
        let action = if key == "bar" { "transport" } else { key };
        let id = if key == "bar" { " id=bar" } else { "" };
        h += &format!(
            "<div class='display {key}-display'><span class=transport-label>{label}</span><span{id} class=led>{}</span>{}{}</div>",
            digits(value, 3), btn(&format!("{action}:-1"), "−"), btn(&format!("{action}:1"), "+")
        );
    }
    h += &format!("<button data-action=loop class='loop-control {}'>Loop</button></header>", if s.looping { "lit" } else { "" });
    h
}
fn render(a: &App) {
    let saved = doc().get_element_by_id("saves")
        .and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()).map(|e| e.value());
    let focus = doc().active_element().filter(|e| !e.id().is_empty()).map(|e| e.id());
    let s = &a.song;
    let mut h=format!("<header class=dialog-header><h2 id=song-title>SONG / FILE · LIBRARY</h2><button data-action=song_close aria-label='Close Song / File'>CLOSE</button></header><div id=song-status class=dialog-status role=status aria-live=polite></div><div class=tools><label>SONG <input id=name value='{}'></label>{}{}<select id=saves aria-label='Saved songs'>{}</select>{}{}<label>IMPORT <input id=import type=file accept='.rbs,.json'></label>{}{}{}{}<div class=chips>",esc(&s.name),btn("save","SAVE"),btn("download","DOWNLOAD JSON"),saved_options(),btn("load","LOAD"),btn("delete","DELETE"),btn("new","NEW"),btn("scene_add","ADD SCENE"),btn("scene_remove","REMOVE LAST"),btn("scene_clear","CLEAR SCENES"));
    for t in 0..4 {
        h += &format!("<div class=pattern-utilities>TRACK {} ", t + 1);
        for (action, label) in [
            ("copy", "COPY"),
            ("paste", "PASTE"),
            ("random", "RANDOM"),
            ("clear", "CLEAR"),
        ] {
            h += &btn(&format!("{action}:{t}"), label);
        }
        h.push_str("</div>");
    }
    h += &format!("<div class=supplemental-controls>{}{}{}</div>",
        toggle("vintage", "VINTAGE", s.vintage),
        toggle("metronome", "METRONOME", a.metronome),
        knob("DELAY WET", "fx:delay_wet", s.fx.delay_wet, 0., 1., 0.01, "black"));
    for (action, label) in [
        ("measure_copy", "COPY LOOP"),
        ("measure_cut", "CUT LOOP"),
        ("measure_insert", "INSERT LOOP"),
        ("measure_replace", "REPLACE LOOP"),
        ("wav", "EXPORT LOOP WAV"),
    ] {
        h += &btn(action, label);
    }
    h+=&format!("<div class=midi-controls>{}<label>MIDI INPUT <select id=midi-device><option value=''>ALL INPUTS</option>{}</select></label><label>CHANNEL <select id=midi-channel>{}</select></label>{}</div>",toggle("midi_enable",if a.midi_enabled{"MIDI ON"}else{"ENABLE MIDI"},a.midi_enabled),a.midi_inputs.iter().map(|(id,name)|format!("<option value='{}' {}>{}</option>",esc(id),if *id==a.midi_device{"selected"}else{""},esc(name))).collect::<String>(),(0..16).map(|n|format!("<option value='{n}' {}>{}</option>",if n==a.midi_channel{"selected"}else{""},n+1)).collect::<String>(),toggle("midi_sync","MIDI SYNC",a.midi_sync));
    let at = a.position / 16;
    let begin = at
        .saturating_sub(32)
        .min(s.arrangement.len().saturating_sub(64));
    for i in begin..(begin + 64).min(s.arrangement.len()) {
        h += &format!(
            "<button data-action='scene:{i}' class='{}'>{}</button>",
            if i == at { "active" } else { "" },
            i + 1
        )
    }
    h.push_str("</div></div>");
    if let Some(dialog) = doc().get_element_by_id("song-dialog") {
        dialog.set_inner_html(&h);
        if let Some(value) = saved.filter(|v| !v.is_empty()) {
            if let Some(select) = doc().get_element_by_id("saves").and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()) {
                select.set_value(&value);
            }
        }
        if dialog.has_attribute("open") {
            fit_song_modal();
            if let Some(el) = focus.and_then(|id| doc().get_element_by_id(&id)).and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
                let _ = el.focus();
            }
        }
    }
    let mut h = String::from("<button id=song-open class=song-launcher data-action=song_open aria-haspopup=dialog aria-controls=song-dialog>SONG / FILE · LIBRARY</button>");
    h += &transport(a);
    h.push_str("<div id=status class=status aria-live=polite></div><main class=rack><div class=tracks>");
    for t in 0..4 {
        h += &format!("<section class='track {}'>", if a.track == t { "focused" } else { "" });
        h += &pattern_box(a, t);
        h += &if t < 2 {
            synth_panel(a, t)
        } else {
            drum_panel(a, t - 2)
        };
        h += &mixer(a, t);
        h.push_str("</section>");
    }
    h.push_str("</div>");
    h += &master(a);
    h.push_str("</main>");
    if let Some(e) = doc().get_element_by_id("app") {
        e.set_inner_html(&h);
    }
    highlight(a.position);
}
fn song_modal(open: bool) -> Result<(), JsValue> {
    let dialog = doc().get_element_by_id("song-dialog").ok_or_else(|| JsValue::from_str("Song dialog is missing"))?;
    if dialog.has_attribute("open") != open {
        let method = if open { "showModal" } else { "close" };
        js_sys::Reflect::get(&dialog, &method.into())?.dyn_into::<js_sys::Function>()?.call0(&dialog)?;
    }
    if open {
        fit_song_modal();
    }
    Ok(())
}
fn fit_song_modal() {
    let Some(dialog) = doc().get_element_by_id("song-dialog").and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) else { return };
    if !dialog.has_attribute("open") {
        return;
    }
    let Some(viewport) = doc().document_element() else { return };
    let _ = dialog.set_attribute("style", "zoom:1");
    let width = dialog.offset_width().max(1) as f64;
    let height = dialog.offset_height().max(1) as f64;
    let scale = ((viewport.client_width() - 32).max(1) as f64 / width)
        .min((viewport.client_height() - 32).max(1) as f64 / height)
        .min(1.);
    let _ = dialog.set_attribute("style", &format!("zoom:{scale}"));
}
fn saved_options() -> String {
    let mut out = String::new();
    if let Ok(Some(store)) = web_sys::window().unwrap().local_storage() {
        for i in 0..store.length().unwrap_or(0) {
            if let Ok(Some(k)) = store.key(i) {
                if let Some(name) = k.strip_prefix("rb338:") {
                    out += &format!("<option value='{}'>{}</option>", esc(name), esc(name));
                }
            }
        }
    }
    out
}
fn highlight(pos: usize) {
    if let Some(e) = doc().get_element_by_id("position") {
        e.set_text_content(Some(&format!(
            "Bar {} · step {}",
            pos / 16 + 1,
            pos % 16 + 1
        )));
    }
    if let Ok(list) = doc().query_selector_all("[data-step]") {
        for i in 0..list.length() {
            if let Some(e) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                let current = e
                    .get_attribute("data-step")
                    .and_then(|s| s.parse::<usize>().ok())
                    == Some(pos % 16);
                let _ = if current {
                    e.class_list().add_1("current")
                } else {
                    e.class_list().remove_1("current")
                };
            }
        }
    }
}
// Reflect engine automation without modifying the editable song or dispatching input events.
fn show_automation(state: &serde_json::Value) {
    if let Ok(nodes) = doc().query_selector_all("[data-key]") {
        for i in 0..nodes.length() {
            let Some(el) = nodes
                .item(i)
                .and_then(|x| x.dyn_into::<HtmlInputElement>().ok())
            else {
                continue;
            };
            if doc()
                .active_element()
                .map(|x| x.is_same_node(Some(&el)))
                .unwrap_or(false)
            {
                continue;
            }
            let Some(key) = el.get_attribute("data-key") else {
                continue;
            };
            let p: Vec<&str> = key.split(':').collect();
            let path = match p.as_slice() {
                ["synth", i, f] => format!("/synth/{i}/{f}"),
                ["drum", k, i, f] => format!("/drums/{k}/voices/{i}/{f}"),
                ["kit", k, "flam"] => format!("/drums/{k}/flam"),
                ["channel", i, f] => format!("/channels/{i}/{f}"),
                ["fx", f] => format!("/fx/{f}"),
                _ => continue,
            };
            if let Some(v) = state.pointer(&path).and_then(|x| x.as_f64()) {
                el.set_value(&v.to_string());
                if let Some(parent) = el.parent_element() {
                    if parent.class_list().contains("knob") {
                        let min = el.min().parse::<f64>().unwrap_or(0.);
                        let max = el.max().parse::<f64>().unwrap_or(1.);
                        if max > min {
                            let _ = parent.set_attribute(
                                "style",
                                &format!("--angle:{}deg", -135. + 270. * (v - min) / (max - min)),
                            );
                        }
                    }
                }
            }
        }
    }
    let show = |action: &str, on: bool| {
        if let Ok(Some(el)) = doc().query_selector(&format!("[data-action='{action}']")) {
            let _ = if on {
                el.class_list().add_1("lit")
            } else {
                el.class_list().remove_1("lit")
            };
        }
    };
    for i in 0..4 {
        for (field, action) in [
            ("distortion", "dist"),
            ("filter", "filter"),
            ("compressor", "comp"),
        ] {
            if let Some(v) = state
                .pointer(&format!("/channels/{i}/{field}"))
                .and_then(|x| x.as_bool())
            {
                show(&format!("{action}:{i}"), v)
            }
        }
        if let Some(v) = state
            .pointer(&format!("/enabled/{i}"))
            .and_then(|x| x.as_bool())
        {
            show(&format!("enable:{i}"), v)
        }
    }
    for f in [
        "filter_enabled",
        "delay_enabled",
        "delay_triplet",
        "distortion_enabled",
        "compressor_enabled",
    ] {
        if let Some(v) = state.pointer(&format!("/fx/{f}")).and_then(|x| x.as_bool()) {
            show(&format!("fx_toggle:{f}"), v)
        }
    }
    if let Some(v) = state
        .pointer("/fx/compressor_master")
        .and_then(|x| x.as_bool())
    {
        show("master_comp", v)
    }
    for (path, id) in [
        ("/fx/filter_pattern", "pcf-number"),
        ("/fx/delay_steps", "delay-number"),
    ] {
        if let Some(v) = state.pointer(path).and_then(|v| v.as_u64()) {
            if let Some(el) = doc().get_element_by_id(id) {
                el.set_inner_html(&digits(v as usize, 2));
            }
        }
    }
    if let Some(v) = state.pointer("/fx/filter_mode").and_then(|v| v.as_str()) {
        if let Ok(Some(el)) = doc().query_selector("[data-action=filter_mode]") {
            el.set_text_content(Some(v));
            let _ = el.set_attribute("data-mode", v);
        }
    }
    for i in 3..=7 {
        if let Some(v) = state
            .pointer(&format!("/drums/0/voices/{i}/variant"))
            .and_then(|v| v.as_bool())
        {
            let action = format!("variant:0:{i}");
            show(&action, v);
            if let Ok(Some(el)) = doc().query_selector(&format!("[data-action='{action}']")) {
                let names = if v {
                    ["CONGA", "CONGA", "CONGA", "CLAVES", "MARACAS"]
                } else {
                    ["TOM", "TOM", "TOM", "RIM", "CLAP"]
                };
                el.set_text_content(Some(names[i - 3]));
            }
        }
    }
    for i in 0..2 {
        if let Some(v) = state
            .pointer(&format!("/synth/{i}/wave"))
            .and_then(|x| x.as_str())
        {
            if let Ok(Some(el)) = doc().query_selector(&format!("[data-action='wave:{i}']")) {
                let _ = el.set_attribute("data-wave", v);
                el.set_text_content(Some(if v == "saw" {
                    "∿ ▮ ▱"
                } else {
                    "∿ ▯ ▰"
                }));
            }
        }
    }
}
struct Punch {
    old: Vec<rb_model::Automation>,
    fresh: Vec<rb_model::Automation>,
    touched: HashSet<(u8, u8)>,
    last: usize,
}
impl Punch {
    fn new(song: &Song, tick: usize) -> Self {
        Self {
            old: song.automation.clone(),
            fresh: Vec::new(),
            touched: HashSet::new(),
            last: tick,
        }
    }
    fn rebuild(&self, song: &mut Song) {
        song.automation = self.old.iter().chain(&self.fresh).cloned().collect();
        song.automation.sort_by_key(|e| e.tick);
    }
    fn event(&mut self, song: &mut Song, tick: usize, track: u8, controller: u8, value: u8) {
        self.touched.insert((track, controller));
        self.old.retain(|e| {
            !(e.tick as usize == tick && e.track == track && e.controller == controller)
        });
        self.fresh.retain(|e| {
            !(e.tick as usize == tick && e.track == track && e.controller == controller)
        });
        self.fresh.push(rb_model::Automation {
            tick: tick as u32,
            track,
            controller,
            value,
        });
        self.last = tick;
        self.rebuild(song);
    }
    fn advance(&mut self, song: &mut Song, tick: usize) -> bool {
        let before = self.old.len();
        let last = self.last;
        let start = song.start * 32;
        let end = song.start.saturating_add(song.length) * 32;
        let wrapped = song.looping && tick < last && tick >= start && last < end;
        self.old.retain(|e| {
            let x = e.tick as usize;
            let crossed = if wrapped {
                (x > last && x < end) || (x >= start && x <= tick)
            } else {
                tick >= last && x > last && x <= tick
            };
            !(crossed && self.touched.contains(&(e.track, e.controller)))
        });
        self.last = tick;
        if self.old.len() != before {
            self.rebuild(song);
            true
        } else {
            false
        }
    }
}
fn end_recording(a: &mut App) {
    if let Some(mut p) = a.punch.take() {
        p.advance(&mut a.song, a.tick);
    }
    a.recording = false;
}
fn request_record_end(a: &mut App) {
    if !a.recording {
        return;
    }
    a.recording = false;
    if a.audio_ready {
        a.next_request = a.next_request.wrapping_add(1);
        let id = a.next_request;
        a.pending_ack = Some(id);
        record_command(0., id);
    } else {
        end_recording(a);
        sync(a);
    }
}
fn replace_song(a: &mut App, song: Song) {
    command(0, 0.);
    a.playing = false;
    request_record_end(a);
    if a.pending_ack.is_some() {
        a.pending_song = Some(song);
    } else {
        install_song(a, song);
    }
}
fn install_song(a: &mut App, song: Song) {
    reset_song_state(a, song);
    sync(a);
    command(5, 0.);
}
fn reset_song_state(a: &mut App, song: Song) {
    a.song = song;
    a.live_controls = None;
    a.position = a.song.start * 16;
    a.tick = a.song.start * 32;
    reset_editor(a);
}
fn audio_started(a: &mut App) {
    if !a.audio_ready {
        command(2, (a.position / 16) as f64);
        a.audio_ready = true;
    }
}
fn record_control(a: &mut App, key: &str, v: f64) {
    let p: Vec<&str> = key.split(':').collect();
    let n = |i: usize| p.get(i).and_then(|x| x.parse::<usize>().ok()).unwrap_or(0);
    let (track, controller, value) = match p[0] {
        "synth" => {
            let c = match p[2] {
                "tune" => 2,
                "cutoff" => 3,
                "resonance" => 4,
                "envelope" => 5,
                "decay" => 6,
                "accent" => 7,
                _ => return,
            };
            let x = match c {
                2 => (v + 12.) / 24.,
                3 => (v - 100.) / 5900.,
                4 => v,
                5 => v / 5000.,
                6 => (v - 0.05) / 0.75,
                _ => v,
            };
            (1 + n(1) as u8, c, (x * 127.).round() as u8)
        }
        "channel" => {
            let i = n(1);
            let c = match p[2] {
                "level" => 6,
                "pan" => 7,
                "delay" => 8,
                _ => return,
            };
            let x = if c == 7 { (v + 1.) / 2. } else { v };
            (0, (c + i * 6) as u8, (x * 127.).round() as u8)
        }
        "drum" => {
            let k = n(1);
            let id = n(2);
            let field = p[3];
            let names = [
                "level", "tune", "decay", "tone", "snappy", "attack", "variant",
            ];
            let Some(f) = names.iter().position(|x| *x == field) else {
                return;
            };
            let maps: [&[(usize, usize)]; 2] = [
                &[
                    (0, 0),
                    (1, 0),
                    (1, 3),
                    (1, 2),
                    (2, 0),
                    (2, 1),
                    (2, 4),
                    (3, 0),
                    (3, 1),
                    (3, 6),
                    (4, 0),
                    (4, 1),
                    (4, 6),
                    (5, 0),
                    (5, 1),
                    (5, 6),
                    (6, 0),
                    (6, 6),
                    (7, 0),
                    (7, 6),
                    (8, 0),
                    (9, 0),
                    (9, 3),
                    (9, 2),
                    (10, 0),
                    (10, 2),
                    (11, 0),
                ],
                &[
                    (0, 0),
                    (1, 0),
                    (1, 1),
                    (1, 5),
                    (1, 2),
                    (2, 0),
                    (2, 1),
                    (2, 3),
                    (2, 4),
                    (3, 0),
                    (3, 1),
                    (3, 2),
                    (4, 0),
                    (4, 1),
                    (4, 2),
                    (5, 0),
                    (5, 1),
                    (5, 2),
                    (6, 0),
                    (7, 0),
                    (8, 0),
                    (8, 2),
                    (9, 2),
                    (10, 0),
                    (10, 1),
                    (11, 0),
                    (11, 1),
                ],
            ];
            let Some(c) = maps[k.min(1)].iter().position(|x| *x == (id, f)) else {
                return;
            };
            let x = match f {
                1 => (v + 6.) / 12.,
                2 => (v - 0.3) / 1.7,
                _ => v,
            };
            (3 + k as u8, (c + 2) as u8, (x * 127.).round() as u8)
        }
        "kit" if p[2] == "flam" => (4, 29, (((v - 0.005) / 0.095) * 127.).round() as u8),
        "fx" => {
            let (t, c, x) = match p[1] {
                "delay_feedback" => (5, 3, v),
                "delay_pan" => (5, 4, (v + 1.) / 2.),
                "distortion" => (6, 1, v),
                "distortion_shape" => (6, 2, v),
                "filter_cutoff" => (7, 1, (v - 100.) / 7900.),
                "filter_q" => (7, 2, (v - 0.5) / 14.5),
                "filter_amount" => (7, 3, v),
                "filter_decay" => (7, 5, (v - 0.1) / 9.9),
                "compressor_ratio" => (8, 1, (v - 1.) / 19.),
                "compressor_threshold" => (8, 2, 10f64.powf(v / 20.)),
                _ => return,
            };
            (t, c, (x * 127.).round() as u8)
        }
        _ => return,
    };
    record_event(a, track, controller, value.min(127));
}
fn record_event(a: &mut App, track: u8, controller: u8, value: u8) {
    if a.audio_ready && a.song.song_mode {
        command(7, ((track as u32) << 8 | controller as u32) as f64);
    }
    if !(a.recording && a.song.song_mode) {
        return;
    }
    if let Some(p) = a.punch.as_mut() {
        p.advance(&mut a.song, a.tick);
        p.event(&mut a.song, a.tick, track, controller, value)
    }
}
fn control_action(song: &mut Song, live: &mut ControlState, parts: &[&str]) {
    let idx = |n: usize| parts.get(n).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    match parts[0] {
        "enable" => {
            let t = idx(1).min(3);
            live.enabled[t] = !live.enabled[t];
            song.enabled[t] = live.enabled[t];
        }
        "wave" => {
            let t = idx(1).min(1);
            live.synth[t].wave = if live.synth[t].wave == "saw" { "square" } else { "saw" }.into();
            song.synth[t].wave = live.synth[t].wave.clone();
        }
        "variant" => {
            let k = idx(1).min(1);
            let v = idx(2).min(11);
            live.drums[k].voices[v].variant = !live.drums[k].voices[v].variant;
            song.drums[k].voices[v].variant = live.drums[k].voices[v].variant;
        }
        "dist" => {
            let t = idx(1).min(3);
            live.channels[t].distortion = !live.channels[t].distortion;
            song.channels[t].distortion = live.channels[t].distortion;
        }
        "filter" => {
            let t = idx(1).min(3);
            let on = !live.channels[t].filter;
            for i in 0..4 {
                live.channels[i].filter = on && i == t;
                song.channels[i].filter = live.channels[i].filter;
            }
        }
        "comp" | "master_comp" => {
            let t = idx(1).min(3);
            let master = parts[0] == "master_comp";
            let on = if master { !live.fx.compressor_master } else { !live.channels[t].compressor };
            for i in 0..4 {
                live.channels[i].compressor = !master && on && i == t;
                song.channels[i].compressor = live.channels[i].compressor;
            }
            live.fx.compressor_master = master && on;
            song.fx.compressor_master = live.fx.compressor_master;
        }
        "fx_toggle" => {
            let (target, value) = match parts[1] {
                "delay_enabled" => (&mut song.fx.delay_enabled, &mut live.fx.delay_enabled),
                "delay_triplet" => (&mut song.fx.delay_triplet, &mut live.fx.delay_triplet),
                "filter_enabled" => (&mut song.fx.filter_enabled, &mut live.fx.filter_enabled),
                "distortion_enabled" => (&mut song.fx.distortion_enabled, &mut live.fx.distortion_enabled),
                "compressor_enabled" => (&mut song.fx.compressor_enabled, &mut live.fx.compressor_enabled),
                _ => return,
            };
            *value = !*value;
            *target = *value;
        }
        "filter_mode" => {
            live.fx.filter_mode = if live.fx.filter_mode == "LP" { "BP" } else { "LP" }.into();
            song.fx.filter_mode = live.fx.filter_mode.clone();
        }
        "fx_step" => {
            let d = parts[2].parse::<isize>().unwrap_or(0);
            if parts[1] == "delay_steps" {
                live.fx.delay_steps = (live.fx.delay_steps as isize + d).clamp(1, 32) as usize;
                song.fx.delay_steps = live.fx.delay_steps;
            } else {
                live.fx.filter_pattern = (live.fx.filter_pattern as isize + d).clamp(0, 54) as usize;
                song.fx.filter_pattern = live.fx.filter_pattern;
            }
        }
        _ => {}
    }
}
fn edit(a: &mut App, key: &str, value: &str, checked: bool) -> bool {
    let parts: Vec<&str> = key.split(':').collect();
    let mut path = String::new();
    let mut text = false;
    let mut boolean = false;
    let mut v = value.parse::<f64>().unwrap_or(0.);
    match parts[0] {
        "bpm" | "shuffle" | "length" => path = format!("/{}", parts[0]),
        "song_mode" | "looping" => {
            path = format!("/{}", parts[0]);
            boolean = true
        }
        "start" => {
            path = "/start".into();
            v -= 1.
        }
        "seek" => {
            command(2, v.max(1.) - 1.);
            return false;
        }
        "slot" => {
            path = format!("/slots/{}", a.track);
            v -= 1.
        }
        "steps" => path = format!("/patterns/{}/lengths/{}", a.song.slots[a.track], a.track),
        "pitch" => {
            path = format!(
                "/patterns/{}/notes/{}/{}/note",
                a.song.slots[a.track], a.track, parts[1]
            )
        }
        "scene" => {
            path = format!("/arrangement/{}/{}", parts[1], parts[2]);
            v -= 1.
        }
        "synth" => {
            path = format!("/synth/{}/{}", parts[1], parts[2]);
            text = parts[2] == "wave"
        }
        "drum" => {
            path = format!("/drums/{}/voices/{}/{}", parts[1], parts[2], parts[3]);
        }
        "kit" => {
            path = if parts[2] == "flam" {
                format!("/drums/{}/flam", parts[1])
            } else {
                format!("/drums/{}/voices/{}/{}", parts[1], a.voice, parts[2])
            };
            boolean = parts[2] == "variant"
        }
        "channel" => {
            path = format!("/channels/{}/{}", parts[1], parts[2]);
            boolean = ["mute", "distortion", "filter", "compressor"].contains(&parts[2])
        }
        "fx" => {
            path = format!("/fx/{}", parts[1]);
            text = parts[1] == "filter_mode";
            boolean = parts[1].ends_with("enabled")
                || ["delay_triplet", "compressor_master"].contains(&parts[1])
        }
        _ => {}
    }
    if path.is_empty() {
        return false;
    }
    let mut json = serde_json::to_value(&a.song).unwrap();
    if let Some(target) = json.pointer_mut(&path) {
        *target = if text {
            value.into()
        } else if boolean {
            checked.into()
        } else if ["slot", "steps", "start", "length", "pitch", "scene"].contains(&parts[0])
            || ["delay_steps", "filter_pattern"].contains(&parts.get(1).copied().unwrap_or(""))
        {
            (v.max(0.) as usize).into()
        } else {
            serde_json::json!(v)
        };
        if let Ok(song) = serde_json::from_value::<Song>(json) {
            if song.validate().is_ok() {
                a.song = song;
                if parts[0] == "drum"
                    && parts[1] == "1"
                    && (parts[2] == "8" || parts[2] == "9")
                    && parts[3] == "level"
                {
                    let other = if parts[2] == "8" { 9 } else { 8 };
                    a.song.drums[1].voices[other].level = v as f32;
                }
                record_control(a, key, v);
                sync(a);
                return ["slot", "steps"].contains(&parts[0]);
            }
        }
    }
    status("Value is outside the allowed range.");
    false
}
fn action(a: &mut App, action: &str) -> bool {
    let parts: Vec<&str> = action.split(':').collect();
    let idx = |n: usize| {
        parts
            .get(n)
            .and_then(|x| x.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let t = if ["copy", "paste", "clear", "random"].contains(&parts[0]) {
        idx(1).min(3)
    } else {
        a.track
    };
    let slot = a.song.slots[t];
    match parts[0] {
        "track" => a.track = idx(1).min(3),
        "voice" => a.voices[idx(1).min(1)] = idx(2).min(11),
        "bank" => a.bank[idx(1).min(3)] = idx(2).min(3),
        "pattern" => {
            let t = idx(1).min(3);
            a.song.slots[t] = a.bank[t] * 8 + idx(2).min(7);
            if a.recording && a.song.song_mode {
                let bar = (a.position / 16).min(4095);
                while a.song.arrangement.len() <= bar {
                    a.song.arrangement.push(a.song.slots);
                }
                a.song.arrangement[bar][t] = a.song.slots[t];
            }
        }
        "enable" | "wave" | "variant" | "dist" | "filter" | "comp" | "master_comp"
        | "fx_toggle" | "filter_mode" | "fx_step" => {
            let mut live = a.live_controls.take().unwrap_or_else(|| a.song.control_state());
            control_action(&mut a.song, &mut live, &parts);
            a.live_controls = Some(live);
        }
        "shuffle_track" => {
            let t = idx(1).min(3);
            a.song.shuffle_tracks[t] = !a.song.shuffle_tracks[t]
        }
        "steps_down" | "steps_up" => {
            let t = idx(1).min(3);
            let n = &mut a.song.patterns[a.song.slots[t]].lengths[t];
            if parts[0] == "steps_up" {
                *n = (*n + 1).min(16)
            } else {
                *n = n.saturating_sub(1).max(1)
            }
        }
        "pitchmode" => {
            let t = idx(1).min(1);
            a.pitch_mode[t] = !a.pitch_mode[t];
            if a.pitch_mode[t] {
                a.edit[t] = 0
            }
        }
        "pitchkey" => {
            let t = idx(1).min(1);
            a.track = t;
            let n = &mut a.song.patterns[a.song.slots[t]].notes[t][a.edit[t]];
            n.note = pitch_with_key(n.note, idx(2).min(12) as u8);
            n.gate = true;
            if a.pitch_mode[t] {
                a.edit[t] = (a.edit[t] + 1) % 16
            }
        }
        "down" | "up" | "note" | "accent" | "slide" => {
            let t = idx(1).min(1);
            let n = &mut a.song.patterns[a.song.slots[t]].notes[t][a.edit[t]];
            match parts[0] {
                "down" => {
                    n.note = toggle_octave(n.note, false);
                }
                "up" => {
                    n.note = toggle_octave(n.note, true);
                }
                "note" => n.gate = !n.gate,
                "accent" => n.accent = !n.accent,
                _ => n.slide = !n.slide,
            }
        }
        "back" | "next" => {
            let t = idx(1).min(1);
            a.edit[t] = (a.edit[t] + if parts[0] == "back" { 15 } else { 1 }) % 16
        }
        "flam" => a.flam = !a.flam,
        "mute" => {
            let t = idx(1).min(3);
            a.song.channels[t].mute = !a.song.channels[t].mute
        }
        "midi_sync" => {
            a.midi_sync = !a.midi_sync;
            command(9, if a.midi_sync { 1. } else { 0. });
        }
        "song_mode" => a.song.song_mode = true,
        "pattern_mode" => a.song.song_mode = false,
        "loop" => a.song.looping = !a.song.looping,
        "vintage" => a.song.vintage = !a.song.vintage,
        "record" => {
            if a.recording {
                request_record_end(a);
            } else {
                if a.pending_ack.is_some() {
                    status("Finishing recording; please wait.");
                    return false;
                }
                a.recording = true;
                if a.song.automation_base.is_none() {
                    a.song.automation_base = Some(a.song.control_state());
                }
                a.punch = Some(Punch::new(&a.song, a.tick));
                a.next_request = a.next_request.wrapping_add(1);
                record_command(1., a.next_request);
            }
        }
        "tempo" => {
            let d = parts[1].parse::<f32>().unwrap_or(0.);
            a.song.bpm = (a.song.bpm + d).clamp(20., 500.)
        }
        "transport" => {
            let d = parts[1].parse::<isize>().unwrap_or(0);
            a.position = ((a.position / 16) as isize + d)
                .max(0)
                .min(a.song.arrangement.len().saturating_sub(1) as isize)
                as usize
                * 16;
            a.tick = a.position * 2;
            if let Some(p) = a.punch.as_mut() {
                p.last = a.tick;
            }
            command(2, (a.position / 16) as f64)
        }
        "start" | "length" => {
            let d = parts[1].parse::<isize>().unwrap_or(0);
            if parts[0] == "start" {
                a.song.start = (a.song.start as isize + d)
                    .max(0)
                    .min(a.song.arrangement.len().saturating_sub(1) as isize)
                    as usize
            } else {
                a.song.length = (a.song.length as isize + d).clamp(1, 4096) as usize
            }
        }
        "scene" => {
            let i = idx(1);
            if let Some(slots) = a.song.arrangement.get(i) {
                a.song.slots = *slots;
                a.position = i * 16;
                a.tick = a.position * 2;
                if let Some(p) = a.punch.as_mut() {
                    p.last = a.tick;
                }
                command(2, i as f64)
            }
        }

        "drum" => {
            let k = idx(1).min(1);
            let v = idx(2).min(11);
            let n = &mut a.song.patterns[a.song.slots[k + 2]].drums[k][v][idx(3).min(15)];
            *n = if k == 1 && a.flam {
                if *n == 3 {
                    0
                } else {
                    3
                }
            } else if k == 1 {
                (*n + 1) % 3
            } else {
                u8::from(*n == 0)
            };
        }
        "copy" => {
            a.clipboard = Some((t, a.song.patterns[slot].clone()));
            status("Pattern copied.");
            return false;
        }
        "paste" => {
            if let Some((source, p)) = &a.clipboard {
                if t < 2 && *source < 2 {
                    a.song.patterns[slot].notes[t] = p.notes[*source].clone()
                } else if t >= 2 && *source >= 2 {
                    a.song.patterns[slot].drums[t - 2] = p.drums[*source - 2].clone()
                } else {
                    status("Copy between matching track types.");
                    return false;
                }
                a.song.patterns[slot].lengths[t] = p.lengths[*source];
            }
        }
        "clear" => {
            if t < 2 {
                for n in &mut a.song.patterns[slot].notes[t] {
                    n.note = 36;
                    n.gate = false;
                    n.accent = false;
                    n.slide = false
                }
            } else {
                for row in &mut a.song.patterns[slot].drums[t - 2] {
                    row.fill(0)
                }
            }
        }
        "random" => {
            if t < 2 {
                for n in &mut a.song.patterns[slot].notes[t] {
                    n.gate = js_sys::Math::random() < 0.7;
                    n.note = 36 + (js_sys::Math::random() * 24.) as u8;
                    n.accent = js_sys::Math::random() < 0.25;
                    n.slide = js_sys::Math::random() < 0.15
                }
            } else {
                for row in &mut a.song.patterns[slot].drums[t - 2] {
                    for n in row {
                        *n = if js_sys::Math::random() < 0.2 {
                            1 + (js_sys::Math::random() * 3.) as u8
                        } else {
                            0
                        }
                    }
                }
            }
        }
        "measure_copy" | "measure_cut" | "measure_insert" | "measure_replace" => {
            if a.recording {
                status("Stop recording before editing measures.");
                return false;
            }
            let result = match parts[0] {
                "measure_copy" => a
                    .song
                    .copy_measures(a.song.start..a.song.start.saturating_add(a.song.length))
                    .map(|clip| a.measure_clipboard = Some(clip)),
                "measure_cut" => a
                    .song
                    .cut_measures(a.song.start..a.song.start.saturating_add(a.song.length))
                    .map(|clip| a.measure_clipboard = Some(clip)),
                _ => match &a.measure_clipboard {
                    Some(clip) => {
                        a.song
                            .paste_measures(a.position / 16, clip, parts[0] == "measure_replace")
                    }
                    None => Err("Copy or cut a loop first.".into()),
                },
            };
            if let Err(e) = result {
                status(&e);
                return false;
            }
        }
        "scene_add" => {
            if a.song.arrangement.len() < 4096 {
                a.song.arrangement.push(a.song.slots)
            }
        }
        "scene_remove" => {
            a.song.arrangement.pop();
            a.song.start = a.song.start.min(a.song.arrangement.len().saturating_sub(1));
        }
        "scene_clear" => {
            a.song.arrangement.clear();
            a.song.start = 0
        }
        "stop" => {
            command(0, 0.);
            a.playing = false;
            request_record_end(a);
        }
        "metronome" => {
            a.metronome = !a.metronome;
            command(4, if a.metronome { 1. } else { 0. });
            status(if a.metronome {
                "Metronome on"
            } else {
                "Metronome off"
            });
            return false;
        }
        "new" => {
            replace_song(a, Song::default());
        }
        "save" => {
            if a.pending_ack.is_some() {
                status("Finishing recording; please wait.");
                return false;
            }
            let name = doc()
                .get_element_by_id("name")
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap()
                .value();
            if name.trim().is_empty() {
                status("Enter a song name.");
                return false;
            }
            a.song.name = name.clone();
            match web_sys::window().unwrap().local_storage() {
                Ok(Some(s)) => {
                    if let Err(e) = s.set_item(
                        &format!("rb338:{name}"),
                        &serde_json::to_string(&a.song).unwrap(),
                    ) {
                        status(&format!("Save failed: {e:?}"));
                        return false;
                    }
                }
                _ => {
                    status("Local storage is unavailable.");
                    return false;
                }
            }
        }
        "load" | "delete" => {
            let name = doc()
                .get_element_by_id("saves")
                .unwrap()
                .dyn_into::<HtmlSelectElement>()
                .unwrap()
                .value();
            if let Ok(Some(store)) = web_sys::window().unwrap().local_storage() {
                let key = format!("rb338:{name}");
                if parts[0] == "delete" {
                    let _ = store.remove_item(&key);
                } else if let Ok(Some(s)) = store.get_item(&key) {
                    match Song::from_json(&s).or_else(|_| rb_model::rbs::import_legacy(&s)) {
                        Ok(song) => {
                            replace_song(a, song);
                        }
                        Err(e) => {
                            status(&e);
                            return false;
                        }
                    }
                } else {
                    status("Saved song not found.");
                    return false;
                }
            } else {
                status("Local storage is unavailable.");
                return false;
            }
        }
        "download" => {
            if a.pending_ack.is_some() {
                status("Finishing recording; please wait.");
                return false;
            }
            let json = serde_json::to_string_pretty(&a.song).unwrap();
            let array = js_sys::Array::new();
            array.push(&JsValue::from_str(&json));
            if let Ok(blob) = web_sys::Blob::new_with_str_sequence(&array) {
                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                    let el = doc()
                        .create_element("a")
                        .unwrap()
                        .dyn_into::<web_sys::HtmlAnchorElement>()
                        .unwrap();
                    el.set_href(&url);
                    el.set_download("rb338-song.json");
                    el.click();
                    let _ = web_sys::Url::revoke_object_url(&url);
                }
            }
            return false;
        }
        _ => return false,
    }
    match parts[0] {
        "enable" => {
            let t = idx(1).min(3);
            record_event(a, (t + 1) as u8, 0, if a.song.enabled[t] { 127 } else { 0 });
        }
        "wave" => {
            let t = idx(1).min(1);
            record_event(
                a,
                (t + 1) as u8,
                8,
                if a.song.synth[t].wave == "square" {
                    127
                } else {
                    0
                },
            );
        }
        "variant" => {
            let k = idx(1).min(1);
            let v = idx(2).min(11);
            let value = if a.song.drums[k].voices[v].variant {
                1.
            } else {
                0.
            };
            record_control(a, &format!("drum:{k}:{v}:variant"), value);
        }
        "dist" => {
            let t = idx(1).min(3);
            record_event(
                a,
                0,
                (9 + 6 * t) as u8,
                if a.song.channels[t].distortion {
                    127
                } else {
                    0
                },
            );
        }
        "filter" => {
            let t = idx(1).min(3);
            record_event(
                a,
                0,
                2,
                if a.song.channels[t].filter {
                    (t + 2) as u8
                } else {
                    0
                },
            );
        }
        "comp" => {
            let t = idx(1).min(3);
            record_event(
                a,
                0,
                1,
                if a.song.channels[t].compressor {
                    (t + 2) as u8
                } else {
                    0
                },
            );
        }
        "master_comp" => record_event(a, 0, 1, if a.song.fx.compressor_master { 1 } else { 0 }),
        "fx_toggle" => {
            let (t, c, on) = match parts[1] {
                "delay_enabled" => (5, 0, a.song.fx.delay_enabled),
                "delay_triplet" => (5, 2, a.song.fx.delay_triplet),
                "filter_enabled" => (7, 0, a.song.fx.filter_enabled),
                "distortion_enabled" => (6, 0, a.song.fx.distortion_enabled),
                "compressor_enabled" => (8, 0, a.song.fx.compressor_enabled),
                _ => (0, 0, false),
            };
            record_event(a, t, c, if on { 127 } else { 0 });
        }
        "filter_mode" => record_event(
            a,
            7,
            6,
            if a.song.fx.filter_mode == "LP" {
                127
            } else {
                0
            },
        ),
        "fx_step" => {
            if parts[1] == "delay_steps" {
                record_event(a, 5, 1, a.song.fx.delay_steps as u8)
            } else {
                record_event(a, 7, 4, a.song.fx.filter_pattern as u8)
            }
        }
        _ => {}
    }
    sync(a);
    true
}
fn midi_panel(a: &mut App, note: u8) -> Option<String> {
    use midi::{KeyAction as K, PatternKey as P, SynthKey as S};
    if let Some(key) = midi::panel_key(note) {
        return match key {
            K::Focus(t) => {
                a.track = t;
                None
            }
            K::Play => Some("play".into()),
            K::Stop => Some("stop".into()),
            K::Record => Some("record".into()),
            K::BarBack => Some("transport:-1".into()),
            K::BarForward => Some("transport:1".into()),
            K::ToggleEffect(i) => Some(format!(
                "fx_toggle:{}",
                [
                    "filter_enabled",
                    "delay_enabled",
                    "distortion_enabled",
                    "compressor_enabled"
                ][i as usize]
            )),
            K::ToggleMix(t) => Some(format!("mute:{t}")),
            K::ToggleDist(t) => Some(format!("dist:{t}")),
            K::ToggleFilter(t) => Some(format!("filter:{t}")),
            K::ToggleCompressor(t) => Some(format!("comp:{t}")),
            K::ToggleMasterCompressor => Some("master_comp".into()),
            K::SelectPatternMode => {
                a.midi_program = false;
                None
            }
            K::ProgramSynthMode => {
                a.midi_program = true;
                None
            }
            K::SelectOrProgram => {
                a.midi_program = !a.midi_program;
                None
            }
        };
    }
    if a.midi_program {
        let t = a.track.min(1);
        return midi::synth_key(note).map(|k| match k {
            S::Pitch(n) => format!("pitchkey:{t}:{n}"),
            S::OctaveDown => format!("down:{t}"),
            S::OctaveUp => format!("up:{t}"),
            S::Accent => format!("accent:{t}"),
            S::Slide => format!("slide:{t}"),
            S::NotePause => format!("note:{t}"),
            S::Back => format!("back:{t}"),
            S::Step => format!("next:{t}"),
            S::PitchMode => format!("pitchmode:{t}"),
        });
    }
    midi::pattern_key(note).map(|k| match k {
        P::Toggle(t) => format!("enable:{t}"),
        P::Bank(t, b) => format!("bank:{t}:{b}"),
        P::Pattern(t, p) => format!("pattern:{t}:{p}"),
    })
}
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    let app = Rc::new(RefCell::new(App {
        song: Song::default(),
        live_controls: None,
        track: 0,
        voice: 0,
        clipboard: None,
        position: 0,
        playing: false,
        metronome: false,
        bank: [0, 0, 0, 0],
        edit: [0, 0],
        pitch_mode: [true, true],
        voices: [1, 1],
        flam: false,
        recording: false,
        tick: 0,
        measure_clipboard: None,
        punch: None,
        exporting: false,
        audio_ready: false,
        pending_ack: None,
        next_request: 0,
        pending_song: None,
        midi_enabled: false,
        midi_sync: false,
        midi_channel: 0,
        midi_program: false,
        midi_device: String::new(),
        midi_inputs: vec![],
        midi_clock: midi::MidiClock::default(),
    }));
    render(&app.borrow());
    let root = doc().get_element_by_id("app").unwrap();
    let song_dialog = doc().get_element_by_id("song-dialog").unwrap();
    let closed = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
        if let Some(button) = doc().get_element_by_id("song-open").and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
            let _ = button.focus();
        }
    });
    song_dialog.add_event_listener_with_callback("close", closed.as_ref().unchecked_ref())?;
    closed.forget();
    let resized = Closure::<dyn FnMut(Event)>::new(move |_: Event| fit_song_modal());
    web_sys::window().unwrap().add_event_listener_with_callback("resize", resized.as_ref().unchecked_ref())?;
    resized.forget();
    let generation = Rc::new(Cell::new(0u32));
    let a = app.clone();
    let generation_click = generation.clone();
    let click = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        let Some(target) = e.target().and_then(|x| x.dyn_into::<Element>().ok()) else {
            return;
        };
        let Ok(Some(el)) = target.closest("[data-action]") else {
            return;
        };
        let Some(act) = el.get_attribute("data-action") else {
            return;
        };
        if act == "song_open" || act == "song_close" {
            if let Err(error) = song_modal(act == "song_open") {
                status(&format!("Song window failed: {error:?}"));
            }
            return;
        }
        if ["stop", "new"].contains(&act.as_str()) {
            generation_click.set(generation_click.get().wrapping_add(1));
        }
        if act == "midi_enable" {
            if a.borrow().midi_enabled {
                close_midi();
                a.borrow_mut().midi_enabled = false;
                render(&a.borrow());
            } else {
                let json = serde_json::to_string(&a.borrow().song).unwrap();
                let audio = start_audio(&json);
                let midi = request_midi();
                match (audio, midi) {
                    (Ok(audio), Ok(midi)) => {
                        let state = a.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let audio = wasm_bindgen_futures::JsFuture::from(audio).await;
                            let midi = wasm_bindgen_futures::JsFuture::from(midi).await;
                            match (audio, midi) {
                                (Ok(_), Ok(_)) => {
                                    {
                                        let mut a = state.borrow_mut();
                                        audio_started(&mut a);
                                        a.midi_enabled = true;
                                        if a.recording {
                                            a.next_request = a.next_request.wrapping_add(1);
                                            record_command(1., a.next_request);
                                        }
                                        command(9, if a.midi_sync { 1. } else { 0. });
                                    }
                                    render(&state.borrow());
                                }
                                (Err(e), _) => status(&format!("Audio setup failed: {e:?}")),
                                (_, Err(e)) => status(&format!("MIDI access failed: {e:?}")),
                            }
                        })
                    }
                    (Err(e), _) => status(&format!("Audio setup failed: {e:?}")),
                    (_, Err(e)) => status(&format!("MIDI access failed: {e:?}")),
                }
            }
            return;
        }
        if act == "wav" {
            if a.borrow().pending_ack.is_some() {
                status("Finishing recording; please wait.");
                return;
            }
            if a.borrow().exporting {
                status("WAV export is already running.");
                return;
            }
            let mut export = match export::WavExport::new(a.borrow().song.clone()) {
                Ok(e) => e,
                Err(e) => {
                    status(&format!("Export failed: {e}"));
                    return;
                }
            };
            a.borrow_mut().exporting = true;
            status("Rendering WAV…");
            let state = a.clone();
            wasm_bindgen_futures::spawn_local(async move {
                while !export.render_chunk(8192) {
                    let pause = js_sys::Promise::new(&mut |resolve, _reject| {
                        let _ = web_sys::window()
                            .unwrap()
                            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0);
                    });
                    let _ = wasm_bindgen_futures::JsFuture::from(pause).await;
                }
                match export.finish() {
                    Ok(bytes) => {
                        let array = js_sys::Array::new();
                        array.push(&js_sys::Uint8Array::from(bytes.as_slice()));
                        match web_sys::Blob::new_with_u8_array_sequence(&array) {
                            Ok(blob) => match web_sys::Url::create_object_url_with_blob(&blob) {
                                Ok(url) => {
                                    if let Ok(el) = doc().create_element("a").and_then(|e| {
                                        e.dyn_into::<web_sys::HtmlAnchorElement>()
                                            .map_err(JsValue::from)
                                    }) {
                                        el.set_href(&url);
                                        el.set_download("rb338-loop.wav");
                                        el.click();
                                    }
                                    let _ = web_sys::Url::revoke_object_url(&url);
                                    status("WAV export ready.")
                                }
                                Err(e) => status(&format!("Export failed: {e:?}")),
                            },
                            Err(e) => status(&format!("Export failed: {e:?}")),
                        }
                    }
                    Err(e) => status(&format!("Export failed: {e}")),
                }
                state.borrow_mut().exporting = false;
            });
            return;
        }
        if act == "play" {
            if a.borrow().playing {
                command(0, 0.);
                {
                    let mut state = a.borrow_mut();
                    state.playing = false;
                    request_record_end(&mut state);
                }
                render(&a.borrow());
            } else {
                let json = serde_json::to_string(&a.borrow().song).unwrap();
                let current = generation_click.get();
                let generation_pending = generation_click.clone();
                match start_audio(&json) {
                    Ok(p) => {
                        let a = a.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match wasm_bindgen_futures::JsFuture::from(p).await {
                                Ok(_) => {
                                    audio_started(&mut a.borrow_mut());
                                    if generation_pending.get() != current {
                                        return;
                                    }
                                    {
                                        let mut state = a.borrow_mut();
                                        if state.recording {
                                            state.next_request = state.next_request.wrapping_add(1);
                                            record_command(1., state.next_request);
                                        }
                                        state.playing = true;
                                    }
                                    command(9, if a.borrow().midi_sync { 1. } else { 0. });
                                    command(1, 0.);
                                    render(&a.borrow());
                                }
                                Err(e) => status(&format!("Audio failed: {e:?}")),
                            }
                        })
                    }
                    Err(e) => status(&format!("Audio failed: {e:?}")),
                }
            }
        } else if act == "load" {
            if action(&mut a.borrow_mut(), &act) {
                generation_click.set(generation_click.get().wrapping_add(1));
                render(&a.borrow());
            }
        } else if act.starts_with("voice:") {
            let fields: Vec<&str> = act.split(':').collect();
            let kit = fields[1].parse::<usize>().unwrap_or(0).min(1);
            let voice = fields[2].parse::<usize>().unwrap_or(0).min(11);
            action(&mut a.borrow_mut(), &act);
            render(&a.borrow());
            if voice > 0 {
                let json = serde_json::to_string(&a.borrow().song).unwrap();
                let current = generation_click.get();
                let generation_pending = generation_click.clone();
                if let Ok(p) = start_audio(&json) {
                    let audition_state = a.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if wasm_bindgen_futures::JsFuture::from(p).await.is_ok()
                            && generation_pending.get() == current
                        {
                            {
                                let mut state = audition_state.borrow_mut();
                                audio_started(&mut state);
                                if state.recording {
                                    state.next_request = state.next_request.wrapping_add(1);
                                    record_command(1., state.next_request);
                                }
                                command(9, if state.midi_sync { 1. } else { 0. });
                            }
                            command(3, (kit * 16 + voice) as f64);
                        }
                    });
                }
            }
        } else if action(&mut a.borrow_mut(), &act) {
            render(&a.borrow());
        }
    });
    root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())?;
    song_dialog.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())?;
    click.forget();
    let a = app.clone();
    let generation_change = generation.clone();
    let change = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        let Some(target) = e.target().and_then(|x| x.dyn_into::<Element>().ok()) else {
            return;
        };
        if target.id() == "import" {
            if e.type_() != "change" {
                return;
            }
            let Some(file) = target
                .dyn_into::<HtmlInputElement>()
                .ok()
                .and_then(|x| x.files())
                .and_then(|x| x.get(0))
            else {
                return;
            };
            let name = file.name().to_lowercase();
            let generation_import = generation_change.clone();
            let a = a.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                    Ok(buf) => {
                        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                        let result = if name.ends_with(".rbs") {
                            rb_model::rbs::import(&bytes)
                        } else {
                            match String::from_utf8(bytes) {
                                Ok(text) => Song::from_json(&text)
                                    .or_else(|_| rb_model::rbs::import_legacy(&text)),
                                Err(e) => Err(e.to_string()),
                            }
                        };
                        match result {
                            Ok(song) => {
                                generation_import.set(generation_import.get().wrapping_add(1));
                                replace_song(&mut a.borrow_mut(), song);
                                render(&a.borrow());
                                status("Song imported.")
                            }
                            Err(e) => status(&format!("Import failed: {e}")),
                        }
                    }
                    Err(e) => status(&format!("Import failed: {e:?}")),
                }
            });
            return;
        }
        if target.has_attribute("data-dial") && e.type_() == "input" {
            if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
                a.borrow_mut().voices[0] = input.value().parse::<usize>().unwrap_or(0).min(11);
                let selected = a.borrow().voices[0];
                if let Some(knob) = target.parent_element() {
                    let _ = knob.set_attribute("style", &format!("--angle:{}deg", 225 + selected * 30));
                }
                if let Some(label) = target
                    .parent_element()
                    .and_then(|x| x.next_element_sibling())
                {
                    label.set_text_content(Some(DRUM_IDS[selected]));
                }
            }
            return;
        }
        if target.has_attribute("data-dial") && e.type_() == "change" {
            render(&a.borrow());
            return;
        }
        if target.id() == "midi-device" && e.type_() == "change" {
            if let Some(x) = target.dyn_ref::<HtmlSelectElement>() {
                let id = x.value();
                select_midi_input(&id);
                a.borrow_mut().midi_device = id;
            }
            return;
        }
        if target.id() == "midi-channel" && e.type_() == "change" {
            if let Some(x) = target.dyn_ref::<HtmlSelectElement>() {
                a.borrow_mut().midi_channel = x.value().parse().unwrap_or(0);
            }
            return;
        }
        if target.id() == "name" {
            if let Some(i) = target.dyn_ref::<HtmlInputElement>() {
                a.borrow_mut().song.name = i.value();
            }
            return;
        }
        let Some(key) = target.get_attribute("data-key") else {
            return;
        };
        if e.type_() != "input" {
            return;
        }
        if key == "seek" {
            generation_change.set(generation_change.get().wrapping_add(1));
        }
        let (value, checked) = if let Some(i) = target.dyn_ref::<HtmlInputElement>() {
            (i.value(), i.checked())
        } else if let Some(s) = target.dyn_ref::<HtmlSelectElement>() {
            (s.value(), false)
        } else {
            return;
        };
        if let Some(i) = target.dyn_ref::<HtmlInputElement>() {
            if let Some(parent) = target.parent_element() {
                if parent.class_list().contains("knob") {
                    let min = i.min().parse::<f64>().unwrap_or(0.);
                    let max = i.max().parse::<f64>().unwrap_or(1.);
                    let v = value.parse::<f64>().unwrap_or(min);
                    let _ = parent.set_attribute(
                        "style",
                        &format!("--angle:{}deg", -135. + 270. * (v - min) / (max - min)),
                    );
                }
            }
        }
        let rerender = edit(&mut a.borrow_mut(), &key, &value, checked);
        if rerender {
            render(&a.borrow())
        } else if let Some(output) = target.next_element_sibling() {
            if output.tag_name() == "OUTPUT" {
                output.set_text_content(Some(&value));
            }
        }
    });
    root.add_event_listener_with_callback("input", change.as_ref().unchecked_ref())?;
    root.add_event_listener_with_callback("change", change.as_ref().unchecked_ref())?;
    song_dialog.add_event_listener_with_callback("input", change.as_ref().unchecked_ref())?;
    song_dialog.add_event_listener_with_callback("change", change.as_ref().unchecked_ref())?;
    change.forget();
    let wheel = Closure::<dyn FnMut(web_sys::WheelEvent)>::new(move |e: web_sys::WheelEvent| {
        let Some(input) = e
            .target()
            .and_then(|x| x.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };
        if !input
            .parent_element()
            .map(|p| p.class_list().contains("knob"))
            .unwrap_or(false)
        {
            return;
        }
        e.prevent_default();
        let min = input.min().parse::<f64>().unwrap_or(0.);
        let max = input.max().parse::<f64>().unwrap_or(1.);
        let step = input.step().parse::<f64>().unwrap_or((max - min) / 100.);
        let value = input.value().parse::<f64>().unwrap_or(min);
        input.set_value(&format!(
            "{}",
            (value + if e.delta_y() < 0. { step } else { -step }).clamp(min, max)
        ));
        if let Ok(event) = {
            let init = web_sys::EventInit::new();
            init.set_bubbles(true);
            Event::new_with_event_init_dict("input", &init)
        } {
            let _ = input.dispatch_event(&event);
        }
    });
    root.add_event_listener_with_callback("wheel", wheel.as_ref().unchecked_ref())?;
    song_dialog.add_event_listener_with_callback("wheel", wheel.as_ref().unchecked_ref())?;
    wheel.forget();
    let drag = Rc::new(RefCell::new(None::<(HtmlInputElement, i32, f64)>));
    let start_drag = drag.clone();
    let down = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
        let Some(input) = e
            .target()
            .and_then(|x| x.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };
        if !input
            .parent_element()
            .map(|p| p.class_list().contains("knob"))
            .unwrap_or(false)
        {
            return;
        }
        e.prevent_default();
        let value = input.value().parse::<f64>().unwrap_or(0.);
        *start_drag.borrow_mut() = Some((input, e.client_y(), value));
    });
    root.add_event_listener_with_callback("mousedown", down.as_ref().unchecked_ref())?;
    song_dialog.add_event_listener_with_callback("mousedown", down.as_ref().unchecked_ref())?;
    down.forget();
    let move_drag = drag.clone();
    let moving = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
        if let Some((input, y, start)) = &*move_drag.borrow() {
            let min = input.min().parse::<f64>().unwrap_or(0.);
            let max = input.max().parse::<f64>().unwrap_or(1.);
            let step = input.step().parse::<f64>().unwrap_or((max - min) / 100.);
            let value = (*start + (*y - e.client_y()) as f64 * (max - min) / 150.).clamp(min, max);
            let value = (min + ((value - min) / step).round() * step).clamp(min, max);
            input.set_value(&format!("{value}"));
            if let Ok(event) = {
                let init = web_sys::EventInit::new();
                init.set_bubbles(true);
                Event::new_with_event_init_dict("input", &init)
            } {
                let _ = input.dispatch_event(&event);
            }
        }
    });
    doc().add_event_listener_with_callback("mousemove", moving.as_ref().unchecked_ref())?;
    moving.forget();
    let up = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |_: web_sys::MouseEvent| {
        if let Some((input, _, _)) = drag.borrow_mut().take() {
            if input.has_attribute("data-dial") {
                let init = web_sys::EventInit::new();
                init.set_bubbles(true);
                if let Ok(event) = Event::new_with_event_init_dict("change", &init) {
                    let _ = input.dispatch_event(&event);
                }
            }
        }
    });
    doc().add_event_listener_with_callback("mouseup", up.as_ref().unchecked_ref())?;
    up.forget();
    let devices_app = app.clone();
    let devices = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        if let Ok(e) = e.dyn_into::<web_sys::CustomEvent>() {
            let list = js_sys::Array::from(&e.detail());
            let mut inputs = Vec::new();
            for item in list.iter() {
                let id = js_sys::Reflect::get(&item, &"id".into())
                    .ok()
                    .and_then(|x| x.as_string());
                let name = js_sys::Reflect::get(&item, &"name".into())
                    .ok()
                    .and_then(|x| x.as_string());
                if let (Some(id), Some(name)) = (id, name) {
                    inputs.push((id, name));
                }
            }
            devices_app.borrow_mut().midi_inputs = inputs;
            render(&devices_app.borrow());
        }
    });
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("rb-midi-devices", devices.as_ref().unchecked_ref())?;
    devices.forget();
    let midi_app = app.clone();
    let midi_event = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        let Ok(e) = e.dyn_into::<web_sys::CustomEvent>() else {
            return;
        };
        let d = e.detail();
        let Ok(bytes) = js_sys::Reflect::get(&d, &"bytes".into()) else {
            return;
        };
        let values = js_sys::Array::from(&bytes);
        let bytes: Vec<u8> = values
            .iter()
            .filter_map(|v| v.as_f64().map(|n| n as u8))
            .collect();
        let Some(event) = midi::decode(&bytes, midi_app.borrow().midi_channel) else {
            return;
        };
        if midi_app.borrow().midi_sync {
            let timestamp = js_sys::Reflect::get(&d, &"timestamp".into())
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.);
            let mut state = midi_app.borrow_mut();
            state.midi_clock.process(&event, timestamp);
            if matches!(event, midi::MidiEvent::Clock) {
                if let Some(bpm) = state.midi_clock.bpm() {
                    let bpm = (bpm as f32).clamp(20., 500.);
                    if (state.song.bpm - bpm).abs() > 0.25 {
                        state.song.bpm = bpm;
                        sync(&state);
                    }
                }
            }
        }
        match event {
            midi::MidiEvent::Control(c) => {
                let mut state = midi_app.borrow_mut();
                rb_model::rbs::apply_automation(&mut state.song, &c);
                record_event(&mut state, c.track, c.controller, c.value);
                sync(&state);
                render(&state)
            }
            midi::MidiEvent::MasterLevel(v) => {
                let mut state = midi_app.borrow_mut();
                state.song.fx.master = v;
                sync(&state);
                render(&state)
            }
            midi::MidiEvent::Shuffle(v) => {
                let mut state = midi_app.borrow_mut();
                state.song.shuffle = v;
                sync(&state);
                render(&state)
            }
            midi::MidiEvent::InstrumentSelect { kit, value } => {
                let mut state = midi_app.borrow_mut();
                state.voices[kit] = ((value as usize * 12) / 128).min(11);
                render(&state)
            }
            midi::MidiEvent::NoteOn { note, .. } => {
                let mapped = midi_panel(&mut midi_app.borrow_mut(), note);
                if let Some(act) = mapped {
                    if act == "play" {
                        if let Ok(Some(el)) = doc().query_selector("[data-action=play]") {
                            if let Ok(button) = el.dyn_into::<web_sys::HtmlElement>() {
                                button.click();
                            }
                        }
                    } else if action(&mut midi_app.borrow_mut(), &act) {
                        render(&midi_app.borrow());
                    }
                }
            }
            midi::MidiEvent::Clock => {
                if midi_app.borrow().midi_sync {
                    command(10, 0.)
                }
            }
            midi::MidiEvent::Start => {
                if midi_app.borrow().midi_sync {
                    command(11, 0.)
                }
            }
            midi::MidiEvent::Continue => {
                if midi_app.borrow().midi_sync {
                    command(12, 0.)
                }
            }
            midi::MidiEvent::Stop => {
                if midi_app.borrow().midi_sync {
                    command(13, 0.);
                    let mut state = midi_app.borrow_mut();
                    state.playing = false;
                    request_record_end(&mut state);
                }
            }
            midi::MidiEvent::SongPosition { sixteenths } => {
                if midi_app.borrow().midi_sync {
                    command(14, sixteenths as f64)
                }
            }
            midi::MidiEvent::NoteOff { .. } => {}
        }
    });
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("rb-midi", midi_event.as_ref().unchecked_ref())?;
    midi_event.forget();
    let a = app.clone();
    let pos = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        if let Ok(e) = e.dyn_into::<web_sys::CustomEvent>() {
            let d = e.detail();
            if let Ok(v) = js_sys::Reflect::get(&d, &"outputMuted".into()) {
                if let Some(v) = v.as_bool() {
                    if let Some(root) = doc().document_element() {
                        let _ = root
                            .set_attribute("data-output-muted", if v { "true" } else { "false" });
                    }
                }
            }
            if let Ok(controls) = js_sys::Reflect::get(&d, &"controls".into()) {
                if !controls.is_null() && !controls.is_undefined() {
                    if let Some(json) = js_sys::JSON::stringify(&controls).ok().and_then(|v| v.as_string()) {
                        if let Ok(state) = serde_json::from_str::<serde_json::Value>(&json) {
                            a.borrow_mut().live_controls = serde_json::from_value(state.clone()).ok();
                            show_automation(&state);
                        }
                    }
                }
            }
            if let Ok(metrics) = js_sys::Reflect::get(&d, &"metrics".into()) {
                for i in 0..6 {
                    if let Ok(value) = js_sys::Reflect::get(&metrics, &JsValue::from_f64(i as f64))
                    {
                        if let Some(value) = value.as_f64() {
                            let id = if i < 2 {
                                format!("meter-{i}")
                            } else {
                                format!("channel-meter-{}", i - 2)
                            };
                            if let Some(m) = doc().get_element_by_id(&id) {
                                let _ = m.set_attribute(
                                    "style",
                                    &format!("--peak:{}%", (value * 100.).clamp(0., 100.)),
                                );
                            }
                        }
                    }
                }
            }
            if let Ok(metrics) = js_sys::Reflect::get(&d, &"metrics".into()) {
                for i in 7..=10 {
                    if let Ok(v) = js_sys::Reflect::get(&metrics, &JsValue::from_f64(i as f64)) {
                        if let Some(v) = v.as_f64() {
                            if let Some(el) = doc().get_element_by_id(&format!("effect-meter-{i}"))
                            {
                                let _ = el.set_attribute(
                                    "style",
                                    &format!("--peak:{}%", (v * 100.).clamp(0., 100.)),
                                );
                            }
                        }
                    }
                }
                if let Ok(v) = js_sys::Reflect::get(&metrics, &JsValue::from_f64(6.)) {
                    if let Some(v) = v.as_f64() {
                        if let Some(el) = doc().get_element_by_id("reduction-meter") {
                            let _ = el.set_attribute(
                                "style",
                                &format!("--peak:{}%", (v.abs() / 24. * 100.).clamp(0., 100.)),
                            );
                            el.set_attribute("title", &format!("Gain reduction: {v:.1} dB"))
                                .ok();
                        }
                    }
                }
            }
            let ack = js_sys::Reflect::get(&d, &"commandAck".into())
                .ok()
                .and_then(|v| v.as_f64());
            let id = js_sys::Reflect::get(&d, &"requestId".into())
                .ok()
                .and_then(|v| v.as_f64())
                .map(|v| v as u32);
            if ack == Some(6.) {
                let mut state = a.borrow_mut();
                if state.pending_ack == id && id.is_some() {
                    if let Ok(t) = js_sys::Reflect::get(&d, &"tick".into()) {
                        if let Some(t) = t.as_f64() {
                            state.tick = t as usize;
                        }
                    }
                    end_recording(&mut state);
                    state.pending_ack = None;
                    sync(&state);
                    if let Some(song) = state.pending_song.take() {
                        install_song(&mut state, song);
                        render(&state);
                        return;
                    }
                }
            }
            if let Ok(t) = js_sys::Reflect::get(&d, &"tick".into()) {
                if let Some(t) = t.as_f64() {
                    let mut state = a.borrow_mut();
                    state.tick = t as usize;
                    if state.recording && state.playing {
                        let tick = state.tick;
                        if let Some(mut punch) = state.punch.take() {
                            let changed = punch.advance(&mut state.song, tick);
                            state.punch = Some(punch);
                            if changed {
                                sync(&state);
                            }
                        }
                    }
                }
            }
            if let Ok(p) = js_sys::Reflect::get(&d, &"position".into()) {
                if let Some(p) = p.as_f64() {
                    a.borrow_mut().position = p as usize;
                    highlight(p as usize);
                    if let Some(e) = doc().get_element_by_id("bar") {
                        e.set_inner_html(&digits(p as usize / 16 + 1, 3));
                    }
                }
            }
            if let Ok(p) = js_sys::Reflect::get(&d, &"playing".into()) {
                if let Some(p) = p.as_bool() {
                    a.borrow_mut().playing = p;
                    if let Ok(Some(button)) = doc().query_selector("[data-action=play]") {
                        let pressed = if p { "true" } else { "false" };
                        if button.get_attribute("aria-pressed").as_deref() != Some(pressed) {
                            button.set_class_name(if p { "lit" } else { "" });
                            button.set_attribute("aria-pressed", pressed).ok();
                        }
                    }
                }
            }
        }
    });
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("rb-position", pos.as_ref().unchecked_ref())?;
    pos.forget();
    let keyboard =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if doc().get_element_by_id("song-dialog").is_some_and(|d| d.has_attribute("open")) {
                return;
            }
            let tag = e
                .target()
                .and_then(|t| t.dyn_into::<Element>().ok())
                .map(|x| x.tag_name())
                .unwrap_or_default();
            if ["INPUT", "SELECT", "TEXTAREA"].contains(&tag.as_str()) || e.repeat() {
                return;
            }
            let key = e.key().to_lowercase();
            if key == " " {
                e.prevent_default();
                if let Ok(Some(el)) = doc().query_selector("[data-action=play]") {
                    if let Ok(button) = el.dyn_into::<web_sys::HtmlElement>() {
                        button.click();
                    }
                }
                return;
            }
            let keys = ["a", "w", "s", "e", "d", "f", "t", "g", "y", "h", "u", "j"];
            if let Some(i) = keys.iter().position(|x| *x == key) {
                let track = app.borrow().track.min(1);
                if let Ok(Some(el)) =
                    doc().query_selector(&format!("[data-action='pitchkey:{track}:{i}']"))
                {
                    if let Ok(button) = el.dyn_into::<web_sys::HtmlElement>() {
                        button.click();
                    }
                }
            }
        });
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("keydown", keyboard.as_ref().unchecked_ref())?;
    keyboard.forget();
    let errors = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
        if let Ok(e) = e.dyn_into::<web_sys::CustomEvent>() {
            status(&format!(
                "Audio error: {}",
                e.detail().as_string().unwrap_or_default()
            ));
        }
    });
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("rb-error", errors.as_ref().unchecked_ref())?;
    errors.forget();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digits_pad_and_keep_only_numeric_text() {
        for (value, width, expected) in [(7, 2, "07"), (42, 3, "042")] {
            let html = digits(value, width);
            let text = html.split('>').skip(1).filter_map(|part| part.chars().next().filter(|c| c.is_ascii_digit())).collect::<String>();
            assert_eq!(text, expected);
            assert_eq!(html.matches("<i></i>").count(), width * 7);
            assert!(!html.contains("<img") && !html.contains("<svg"));
        }
    }
    #[test]
    fn pitch_selection_preserves_bounded_octave() {
        assert_eq!(pitch_with_key(36, 7), 43);
        assert_eq!(pitch_with_key(36, 12), 48);
        assert_eq!(pitch_with_key(24, 7), 31);
        assert_eq!(pitch_with_key(48, 7), 55);
        assert_eq!(toggle_octave(43, false), 31);
        assert_eq!(toggle_octave(31, false), 43);
        assert_eq!(toggle_octave(43, true), 55);
        assert_eq!(toggle_octave(55, true), 43);
    }
    #[test]
    fn punch_removes_old_events_only_for_touched_lanes_across_loop() {
        let mut song = Song::default();
        song.start = 0;
        song.length = 2;
        song.looping = true;
        song.automation = vec![
            rb_model::Automation {
                tick: 4,
                track: 1,
                controller: 3,
                value: 1,
            },
            rb_model::Automation {
                tick: 8,
                track: 1,
                controller: 3,
                value: 2,
            },
            rb_model::Automation {
                tick: 10,
                track: 1,
                controller: 4,
                value: 3,
            },
            rb_model::Automation {
                tick: 62,
                track: 1,
                controller: 3,
                value: 4,
            },
        ];
        let mut punch = Punch::new(&song, 5);
        punch.event(&mut song, 5, 1, 3, 99);
        assert!(punch.advance(&mut song, 12));
        assert_eq!(
            song.automation
                .iter()
                .map(|e| (e.tick, e.controller, e.value))
                .collect::<Vec<_>>(),
            vec![(4, 3, 1), (5, 3, 99), (10, 4, 3), (62, 3, 4)]
        );
        punch.advance(&mut song, 63);
        punch.event(&mut song, 63, 1, 3, 100);
        assert!(punch.advance(&mut song, 5));
        assert_eq!(
            song.automation
                .iter()
                .map(|e| (e.tick, e.controller, e.value))
                .collect::<Vec<_>>(),
            vec![(5, 3, 99), (10, 4, 3), (63, 3, 100)]
        );
    }
    #[test]
    fn buttons_use_live_values_without_overwriting_other_controls() {
        let mut song = Song::default();
        let mut live = song.control_state();
        live.fx.filter_pattern = 20;
        live.fx.delay_steps = 7;
        live.fx.filter_enabled = false;
        live.synth[0].cutoff = 5000.;
        live.synth[0].wave = "square".into();
        live.enabled[0] = false;
        control_action(&mut song, &mut live, &["fx_step", "filter_pattern", "1"]);
        assert_eq!(song.fx.filter_pattern, 21);
        control_action(&mut song, &mut live, &["fx_step", "filter_pattern", "1"]);
        assert_eq!(song.fx.filter_pattern, 22);
        control_action(&mut song, &mut live, &["fx_step", "delay_steps", "-1"]);
        assert_eq!(song.fx.delay_steps, 6);
        control_action(&mut song, &mut live, &["fx_toggle", "filter_enabled"]);
        assert!(song.fx.filter_enabled);
        control_action(&mut song, &mut live, &["enable", "0"]);
        assert!(song.enabled[0]);
        control_action(&mut song, &mut live, &["wave", "0"]);
        assert_eq!(song.synth[0].wave, "saw");
        assert_eq!(song.synth[0].cutoff, Song::default().synth[0].cutoff);
        assert_eq!(live.synth[0].cutoff, 5000.);
        assert!(song.automation.is_empty());
    }
    #[test]
    fn live_routing_buttons_keep_one_destination() {
        let mut song = Song::default();
        let mut live = song.control_state();
        live.channels[2].filter = true;
        live.fx.compressor_master = false;
        live.channels[1].compressor = true;
        control_action(&mut song, &mut live, &["filter", "2"]);
        assert!(song.channels.iter().all(|c| !c.filter));
        control_action(&mut song, &mut live, &["comp", "1"]);
        assert!(song.channels.iter().all(|c| !c.compressor));
        assert!(!song.fx.compressor_master);
        control_action(&mut song, &mut live, &["master_comp"]);
        assert!(song.fx.compressor_master);
        control_action(&mut song, &mut live, &["comp", "3"]);
        assert!(!song.fx.compressor_master);
        assert_eq!(song.channels.each_ref().map(|c| c.compressor), [false, false, false, true]);
    }
    #[test]
    fn replacement_resets_editor_and_live_state_to_incoming_start() {
        let mut a = test_app();
        a.live_controls = Some(a.song.control_state());
        a.position = 64;
        a.tick = 128;
        a.edit = [7, 9];
        let mut song = Song::default();
        song.start = 1;
        song.slots = [8, 17, 26, 3];
        reset_song_state(&mut a, song.clone());
        assert_eq!(a.song, song);
        assert_eq!((a.position, a.tick), (16, 32));
        assert_eq!(a.bank, [1, 2, 3, 0]);
        assert_eq!(a.edit, [0, 0]);
        assert!(a.live_controls.is_none());
    }
    fn test_app() -> App {
        App {
            song: Song::default(),
            live_controls: None,
            track: 0,
            voice: 0,
            clipboard: None,
            position: 16,
            playing: true,
            metronome: false,
            bank: [0; 4],
            edit: [0; 2],
            pitch_mode: [true; 2],
            voices: [1; 2],
            flam: false,
            recording: true,
            tick: 32,
            measure_clipboard: None,
            punch: None,
            exporting: false,
            audio_ready: false,
            pending_ack: None,
            next_request: 0,
            pending_song: None,
            midi_enabled: false,
            midi_sync: false,
            midi_channel: 0,
            midi_program: false,
            midi_device: String::new(),
            midi_inputs: vec![],
            midi_clock: midi::MidiClock::default(),
        }
    }
    #[test]
    fn recording_replaces_same_controller_and_sorts_ticks() {
        let mut a = test_app();
        a.song.song_mode = true;
        a.punch = Some(Punch::new(&a.song, a.tick));
        record_control(&mut a, "synth:0:tune", 12.);
        record_control(&mut a, "synth:0:tune", -12.);
        a.position = 0;
        a.tick = 0;
        record_control(&mut a, "synth:0:cutoff", 6000.);
        assert_eq!(a.song.automation.len(), 2);
        assert_eq!(
            (a.song.automation[0].tick, a.song.automation[0].value),
            (0, 127)
        );
        assert_eq!(
            (a.song.automation[1].tick, a.song.automation[1].value),
            (32, 0)
        );
    }
}
