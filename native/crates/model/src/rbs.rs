use crate::{Automation, Note, Song, DRUM_IDS, DRUM_IDS_B};
use serde_json::Value;

fn unit(v: u8) -> f32 {
    (v.min(127) as f32) / 127.0
}

fn rbs_control_value(track: u8, controller: u8, value: u8) -> u8 {
    if track == 0 && controller == 2 && value != 0 {
        value.saturating_add(1)
    } else if (track == 5 && controller == 2)
        || (track == 3 && matches!(controller, 11 | 14 | 17 | 19 | 21))
    {
        u8::from(value == 0)
    } else {
        value
    }
}

pub fn apply_automation(song: &mut Song, e: &Automation) {
    let u = unit(e.value);
    let c = e.controller as usize;
    let v = e.value;
    match e.track {
        1 | 2 => {
            let i = (e.track - 1) as usize;
            let s = &mut song.synth[i];
            match c {
                0 => song.enabled[i] = v != 0,
                2 => s.tune = (u - 0.5) * 24.,
                3 => s.cutoff = 100. + u * 5900.,
                4 => s.resonance = u,
                5 => s.envelope = u * 5000.,
                6 => s.decay = 0.05 + u * 0.75,
                7 => s.accent = u,
                8 => s.wave = if v == 0 { "saw" } else { "square" }.into(),
                _ => {}
            }
        }
        3 | 4 => {
            let k = (e.track - 3) as usize;
            if c == 0 {
                song.enabled[k + 2] = v != 0;
                return;
            }
            if k == 1 && c == 29 {
                song.drums[k].flam = 0.005 + u * 0.095;
                return;
            }
            let a: [Option<(usize, u8)>; 29] = [
                None,
                None,
                Some((0, 0)),
                Some((1, 0)),
                Some((1, 3)),
                Some((1, 2)),
                Some((2, 0)),
                Some((2, 1)),
                Some((2, 4)),
                Some((3, 0)),
                Some((3, 1)),
                Some((3, 6)),
                Some((4, 0)),
                Some((4, 1)),
                Some((4, 6)),
                Some((5, 0)),
                Some((5, 1)),
                Some((5, 6)),
                Some((6, 0)),
                Some((6, 6)),
                Some((7, 0)),
                Some((7, 6)),
                Some((8, 0)),
                Some((9, 0)),
                Some((9, 3)),
                Some((9, 2)),
                Some((10, 0)),
                Some((10, 2)),
                Some((11, 0)),
            ];
            let b: [Option<(usize, u8)>; 29] = [
                None,
                None,
                Some((0, 0)),
                Some((1, 0)),
                Some((1, 1)),
                Some((1, 5)),
                Some((1, 2)),
                Some((2, 0)),
                Some((2, 1)),
                Some((2, 3)),
                Some((2, 4)),
                Some((3, 0)),
                Some((3, 1)),
                Some((3, 2)),
                Some((4, 0)),
                Some((4, 1)),
                Some((4, 2)),
                Some((5, 0)),
                Some((5, 1)),
                Some((5, 2)),
                Some((6, 0)),
                Some((7, 0)),
                Some((8, 0)),
                Some((8, 2)),
                Some((9, 2)),
                Some((10, 0)),
                Some((10, 1)),
                Some((11, 0)),
                Some((11, 1)),
            ];
            if let Some((i, f)) = if k == 0 { a.get(c) } else { b.get(c) }.copied().flatten() {
                let d = &mut song.drums[k].voices[i];
                match f {
                    0 => d.level = u,
                    1 => d.tune = u * 12. - 6.,
                    2 => d.decay = 0.3 + u * 1.7,
                    3 => d.tone = u,
                    4 => d.snappy = u,
                    5 => d.attack = u,
                    6 => d.variant = v != 0,
                    _ => {}
                }
                if k == 1 && c == 22 {
                    song.drums[k].voices[9].level = u;
                }
            }
        }
        0 => {
            if c == 1 || c == 2 {
                for (i, ch) in song.channels.iter_mut().enumerate() {
                    if c == 1 {
                        ch.compressor = v as usize == i + 2
                    } else {
                        ch.filter = v as usize == i + 2
                    }
                }
                if c == 1 {
                    song.fx.compressor_master = v == 1
                }
                return;
            }
            for i in 0..4 {
                let ch = &mut song.channels[i];
                if c == 6 + i * 6 {
                    ch.level = u
                }
                if c == 7 + i * 6 {
                    ch.pan = u * 2. - 1.
                }
                if c == 8 + i * 6 {
                    ch.delay = u
                }
                if c == 9 + i * 6 {
                    ch.distortion = v != 0
                }
            }
            if c == 32 {
                song.channels[3].delay = u
            }
            if c == 33 {
                song.channels[3].distortion = v != 0
            }
        }
        5 => match c {
            0 => song.fx.delay_enabled = v != 0,
            1 => song.fx.delay_steps = (v as usize).clamp(1, 32),
            2 => song.fx.delay_triplet = v != 0,
            3 => song.fx.delay_feedback = u,
            4 => song.fx.delay_pan = u * 2. - 1.,
            _ => {}
        },
        6 => match c {
            0 => song.fx.distortion_enabled = v != 0,
            1 => song.fx.distortion = u,
            2 => song.fx.distortion_shape = u,
            _ => {}
        },
        7 => match c {
            0 => song.fx.filter_enabled = v != 0,
            1 => song.fx.filter_cutoff = 100. + u * 7900.,
            2 => song.fx.filter_q = 0.5 + u * 14.5,
            3 => song.fx.filter_amount = u,
            4 => song.fx.filter_pattern = (v as usize).min(54),
            5 => song.fx.filter_decay = 0.1 + u * 9.9,
            6 => song.fx.filter_mode = if v == 0 { "BP" } else { "LP" }.into(),
            _ => {}
        },
        8 => match c {
            0 => song.fx.compressor_enabled = v != 0,
            1 => song.fx.compressor_ratio = 1. + u * 19.,
            2 => song.fx.compressor_threshold = (20. * u.max(0.01).log10()).max(-40.),
            _ => {}
        },
        _ => {}
    }
}
fn be(b: &[u8], p: usize) -> Result<usize, String> {
    let s = b
        .get(p..p.checked_add(4).ok_or("overflow")?)
        .ok_or("truncated number")?;
    Ok(u32::from_be_bytes(s.try_into().unwrap()) as usize)
}
fn parse_chunks(
    b: &[u8],
    start: usize,
    end: usize,
    s: &mut Song,
    devices: &mut [bool; 4],
    track: &mut u8,
    changes: &mut [Vec<(usize, usize)>; 4],
    end_tick: &mut usize,
    loop_range: &mut Option<(usize, usize)>,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("container nesting too deep".into());
    }
    let mut at = start;
    while at < end {
        let tag = b
            .get(at..at.checked_add(4).ok_or("overflow")?)
            .ok_or("truncated chunk")?;
        let size = be(b, at + 4)?;
        let body = at.checked_add(8).ok_or("overflow")?;
        let next = body.checked_add(size).ok_or("overflow")?;
        if next > end {
            return Err("chunk exceeds file".into());
        }
        let x = &b[body..next];
        match tag {
            b"CAT " => {
                if size < 4 {
                    return Err("short container".into());
                }
                parse_chunks(
                    b,
                    body + 4,
                    next,
                    s,
                    devices,
                    track,
                    changes,
                    end_tick,
                    loop_range,
                    depth + 1,
                )?
            }
            b"HEAD" => {
                if size < 28 || x.get(..4) != Some(&b"[T[T"[..]) {
                    return Err("bad HEAD".into());
                }
            }
            b"GLOB" => {
                if size < 32 {
                    return Err("short GLOB".into());
                }
                s.bpm = be(x, 2)? as f32 / 1000.;
                s.song_mode = x[0] != 0;
                s.looping = x[1] != 0;
                s.shuffle = unit(x[14]) * 60.;
                s.vintage = x.get(482).copied().unwrap_or(0) != 0;
                let start = be(x, 6)?;
                let end = be(x, 10)?;
                if end == 0 {
                    *loop_range = None;
                } else {
                    if end <= start || start % 768 != 0 || end % 768 != 0 || end / 768 > 4096 {
                        return Err("invalid GLOB loop range; expected whole bars within 4096 bars".into());
                    }
                    *loop_range = Some((start / 768, end / 768));
                }
                s.name = String::from_utf8_lossy(&x[15..])
                    .split('\0')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            }
            b"303 " | b"808 " | b"909 " => {
                let i = if tag == b"303 " {
                    if !devices[0] {
                        0
                    } else {
                        1
                    }
                } else if tag == b"808 " {
                    2
                } else {
                    3
                };
                if devices[i] {
                    return Err("duplicate device".into());
                }
                devices[i] = true;
                let head = if i < 2 {
                    9
                } else if i == 2 {
                    30
                } else {
                    31
                };
                let stride = if i < 2 { 34 } else { 194 };
                if size < head + 32 * stride {
                    return Err("truncated patterns".into());
                }
                if x[1] > 31 {
                    return Err("invalid slot".into());
                }
                s.slots[i] = x[1] as usize;
                s.enabled[i] = x[0] != 0;
                s.shuffle_tracks[i] = x[head + s.slots[i] * stride] != 0;
                for c in 2..head {
                    apply_automation(
                        s,
                        &Automation {
                            tick: 0,
                            track: (i + 1) as u8,
                            controller: c as u8,
                            value: rbs_control_value((i + 1) as u8, c as u8, x[c]),
                        },
                    );
                }
                for slot in 0..32 {
                    let p = head + slot * stride;
                    let len = x[p + 1] as usize;
                    if len > 16 {
                        return Err("invalid pattern length".into());
                    }
                    s.patterns[slot].lengths[i] = if len == 0 { 16 } else { len };
                    for n in 0..16 {
                        if i < 2 {
                            let pitch = x[p + 2 + n * 2];
                            let flags = x[p + 3 + n * 2];
                            if pitch > 12 {
                                return Err("invalid pitch".into());
                            }
                            let octave = if flags & 4 != 0 {
                                12
                            } else if flags & 8 != 0 {
                                -12
                            } else {
                                0
                            };
                            s.patterns[slot].notes[i][n] = Note {
                                note: (36 + pitch as i32 + octave) as u8,
                                gate: flags & 16 == 0 && n < len,
                                accent: flags & 2 != 0,
                                slide: flags & 1 != 0,
                            }
                        } else {
                            for j in 0..12 {
                                s.patterns[slot].drums[i - 2][j][n] = x[p + 2 + n * 12 + j];
                            }
                        }
                    }
                }
            }
            b"TRAK" => {
                if size < 4 {
                    return Err("short track".into());
                }
                let count = be(x, 0)?;
                if count > (size - 4) / 3 {
                    return Err("invalid event count".into());
                }
                let mut p = 4;
                let mut tick = 0usize;
                for _ in 0..count {
                    let mut delta = 0usize;
                    let mut n = 0;
                    loop {
                        let v = *x.get(p).ok_or("truncated time")?;
                        p += 1;
                        n += 1;
                        if n > 5 {
                            return Err("invalid time".into());
                        }
                        delta = delta
                            .checked_mul(128)
                            .and_then(|d| d.checked_add((v & 127) as usize))
                            .ok_or("time overflow")?;
                        if v & 128 == 0 {
                            break;
                        }
                    }
                    tick = tick.checked_add(delta).ok_or("time overflow")?;
                    let c = *x.get(p).ok_or("truncated event")?;
                    let v = *x.get(p + 1).ok_or("truncated event")?;
                    p += 2;
                    if (1..=4).contains(track) && c == 1 {
                        if v > 31 {
                            return Err("invalid pattern".into());
                        }
                        changes[*track as usize - 1].push((tick, v as usize))
                    } else {
                        s.automation.push(Automation {
                            tick: u32::try_from(tick).map_err(|_| "time overflow")?,
                            track: *track,
                            controller: c,
                            value: rbs_control_value(*track, c, v),
                        })
                    }
                }
                if p != size {
                    return Err("extra track data".into());
                }
                *end_tick = (*end_tick).max(tick);
                *track = track.checked_add(1).ok_or("too many tracks")?
            }
            b"MIXR" => {
                if size < 64 {
                    return Err("short mixer".into());
                }
                s.fx.master = unit(x[0]);
                for i in 0..4 {
                    let p = 16 + i * 12;
                    s.channels[i].level = unit(x[p + 1]);
                    s.channels[i].pan = unit(x[p + 2]) * 2. - 1.;
                    s.channels[i].delay = unit(x[p + 3]);
                    s.channels[i].distortion = x[p + 4] != 0;
                    s.channels[i].filter = x[2] as usize == i + 1;
                    s.channels[i].compressor = x[1] as usize == i + 2;
                }
                s.fx.compressor_master = x[1] == 1
            }
            b"DELY" | b"DIST" | b"PCF " | b"COMP" => {
                let (t, min) = if tag == b"DELY" {
                    (5, 5)
                } else if tag == b"DIST" {
                    (6, 3)
                } else if tag == b"PCF " {
                    (7, 7)
                } else {
                    (8, 3)
                };
                if size < min {
                    return Err("short effects".into());
                }
                for c in 0..min {
                    apply_automation(
                        s,
                        &Automation {
                            tick: 0,
                            track: t,
                            controller: c as u8,
                            value: rbs_control_value(t, c as u8, x[c]),
                        },
                    )
                }
                if t == 5 {
                    s.fx.delay_wet = 1.
                }
            }
            _ => {}
        }
        at = next.checked_add(size & 1).ok_or("overflow")?;
        if at > end {
            return Err("missing padding".into());
        }
    }
    Ok(())
}
pub fn import(b: &[u8]) -> Result<Song, String> {
    if b.len() > 16 * 1024 * 1024
        || b.len() < 20
        || b.get(..4) != Some(&b"CAT "[..])
        || b.get(8..12) != Some(&b"RB40"[..])
        || be(b, 4)?.checked_add(8) != Some(b.len())
    {
        return Err("invalid RBS header".into());
    }
    let mut s = Song::default();
    let mut devices = [false; 4];
    let mut track = 0;
    let mut changes = std::array::from_fn(|_| Vec::new());
    let mut end = 0;
    let mut loop_range = None;
    parse_chunks(
        b,
        12,
        b.len(),
        &mut s,
        &mut devices,
        &mut track,
        &mut changes,
        &mut end,
        &mut loop_range,
        0,
    )?;
    if devices.iter().any(|v| !*v) || !(20. ..=500.).contains(&s.bpm) {
        return Err("missing instruments or tempo".into());
    }
    let bars = (end / 32 + usize::from(end != 4096 * 32))
        .max(loop_range.map_or(1, |(_, end)| end));
    if bars > 4096 {
        return Err("song exceeds 4096 bars".into());
    }
    let mut slots = s.slots;
    let mut cursor = [0; 4];
    s.arrangement.clear();
    for bar in 0..bars {
        for i in 0..4 {
            while cursor[i] < changes[i].len() && changes[i][cursor[i]].0 <= bar * 32 {
                slots[i] = changes[i][cursor[i]].1;
                cursor[i] += 1
            }
        }
        s.arrangement.push(slots)
    }
    if let Some((start, end)) = loop_range {
        s.start = start;
        s.length = end - start;
    } else {
        s.start = 0;
        s.length = bars;
    }
    s.automation.sort_by_key(|e| (e.tick, e.track));
    s.validate()?;
    Ok(s)
}
fn number(v: &Value, key: &str, default: f32) -> f32 {
    v.get(key)
        .and_then(Value::as_f64)
        .map(|n| n as f32)
        .unwrap_or(default)
}
fn boolean(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(default)
}
fn index(v: &Value, key: &str, default: usize) -> usize {
    v.get(key)
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(default)
}
pub fn import_legacy(text: &str) -> Result<Song, String> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("song exceeds 16 MB".into());
    }
    let mut v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if v.get("automation")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        if let Some(base) = v.get("autoBase").and_then(Value::as_object).cloned() {
            if let Some(root) = v.as_object_mut() {
                for key in ["t", "v", "drum", "send"] {
                    if let Some(value) = base.get(key) {
                        root.insert(key.into(), value.clone());
                    }
                }
            }
        }
    }
    let mut s = Song::default();
    s.name = v
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Imported song")
        .to_owned();
    s.bpm = number(&v, "bpm", s.bpm);
    s.shuffle = number(&v, "shuf", s.shuffle);
    s.looping = boolean(&v, "loop", s.looping);
    s.song_mode = boolean(&v, "songMode", s.song_mode);
    s.start = index(&v, "start", 1).saturating_sub(1);
    s.length = index(&v, "len", s.length);
    let keys = ["s1", "s2", "d808", "d909"];
    let slots = v.get("slots").ok_or("missing slots")?;
    for i in 0..4 {
        s.slots[i] = index(slots, keys[i], s.slots[i])
    }
    let arr = v
        .get("song")
        .and_then(Value::as_array)
        .ok_or("missing arrangement")?;
    s.arrangement = arr
        .iter()
        .map(|row| std::array::from_fn(|i| index(row, keys[i], 0)))
        .collect();
    let pats = v
        .get("patterns")
        .and_then(Value::as_array)
        .ok_or("missing patterns")?;
    if pats.len() != 32 {
        return Err("expected 32 patterns".into());
    }
    for (slot, p) in pats.iter().enumerate() {
        for k in 0..2 {
            let notes = p
                .get(keys[k])
                .and_then(Value::as_array)
                .ok_or("missing notes")?;
            if notes.len() != 16 {
                return Err("expected 16 notes".into());
            }
            for (i, n) in notes.iter().enumerate() {
                let note = index(n, "note", 48);
                if note > 127 {
                    return Err("invalid note".into());
                }
                s.patterns[slot].notes[k][i] = Note {
                    note: note as u8,
                    gate: boolean(n, "gate", false),
                    accent: boolean(n, "accent", false),
                    slide: boolean(n, "slide", false),
                }
            }
        }
        for k in 0..2 {
            let ids = if k == 0 { DRUM_IDS } else { DRUM_IDS_B };
            let drums = p.get(keys[k + 2]).ok_or("missing drums")?;
            for (j, id) in ids.iter().enumerate() {
                let name = id.to_ascii_lowercase();
                if k == 1 && name == "ac" && drums.get(&name).is_none() {
                    s.patterns[slot].drums[k][j].fill(0);
                    continue;
                }
                let row = drums
                    .get(&name)
                    .and_then(Value::as_array)
                    .ok_or("missing drum row")?;
                if row.len() != 16 {
                    return Err("expected 16 steps".into());
                }
                for (i, hit) in row.iter().enumerate() {
                    let n = hit
                        .as_u64()
                        .or_else(|| hit.as_bool().map(u64::from))
                        .ok_or("invalid drum hit")?;
                    if n > 3 {
                        return Err("invalid drum hit".into());
                    }
                    s.patterns[slot].drums[k][j][i] = n as u8
                }
            }
        }
        for i in 0..4 {
            let n = v
                .get("plens")
                .and_then(|x| x.get(keys[i]))
                .and_then(Value::as_array)
                .and_then(|x| x.get(slot))
                .and_then(Value::as_u64)
                .map(|x| x as usize)
                .unwrap_or_else(|| v.get("plen").map(|p| index(p, keys[i], 16)).unwrap_or(16));
            s.patterns[slot].lengths[i] = n
        }
    }
    if let Some(t) = v.get("t") {
        for (i, key) in ["a", "b"].iter().enumerate() {
            if let Some(x) = t.get(key) {
                let z = &mut s.synth[i];
                z.wave = match x.get("wave").and_then(Value::as_str).unwrap_or("sawtooth") {
                    "sawtooth" => "saw",
                    a => a,
                }
                .into();
                z.tune = number(x, "tune", z.tune);
                z.cutoff = number(x, "cutoff", z.cutoff);
                z.resonance = number(x, "res", z.resonance * 20.) / 20.;
                z.envelope = number(x, "env", z.envelope);
                z.decay = number(x, "decay", z.decay);
                z.accent = number(x, "accent", z.accent);
                s.channels[i].mute = boolean(x, "mute", s.channels[i].mute)
            }
        }
        s.channels[2].mute = boolean(t, "m808", s.channels[2].mute);
        s.channels[3].mute = boolean(t, "m909", s.channels[3].mute)
    }
    if let Some(d) = v.get("drum") {
        for k in 0..2 {
            let ids = if k == 0 { DRUM_IDS } else { DRUM_IDS_B };
            if let Some(kit) = d.get(if k == 0 { "808" } else { "909" }) {
                s.drums[k].flam = number(kit, "flam", s.drums[k].flam);
                for (i, id) in ids.iter().enumerate() {
                    let key = id.to_ascii_lowercase();
                    let voice = &mut s.drums[k].voices[i];
                    for (field, target) in [
                        ("lvl", &mut voice.level),
                        ("tune", &mut voice.tune),
                        ("dec", &mut voice.decay),
                        ("tone", &mut voice.tone),
                        ("snappy", &mut voice.snappy),
                        ("attack", &mut voice.attack),
                    ] {
                        if let Some(n) = kit
                            .get(field)
                            .and_then(|x| x.get(&key))
                            .and_then(Value::as_f64)
                        {
                            *target = n as f32
                        }
                    }
                    if k == 0 && (i == 1 || i == 9) {
                        if let Some(n) = kit
                            .get("tune")
                            .and_then(|x| x.get(&key))
                            .and_then(Value::as_f64)
                        {
                            voice.tone = ((n as f32 + 6.) / 12.).clamp(0., 1.);
                        }
                    }
                    voice.variant = kit
                        .get("select")
                        .and_then(|x| x.get(&key))
                        .and_then(Value::as_bool)
                        .unwrap_or(voice.variant)
                }
            }
        }
    }
    if let Some(x) = v.get("v") {
        for (i, key) in ["a", "b", "s808", "s909"].iter().enumerate() {
            s.channels[i].level = number(x, key, s.channels[i].level);
            if let Some(p) = x.get("pan") {
                s.channels[i].pan = number(p, key, s.channels[i].pan)
            }
        }
        let f = &mut s.fx;
        f.master = number(x, "master", f.master);
        f.delay_enabled = boolean(x, "dmode", f.delay_enabled);
        f.delay_steps = index(x, "dsteps", f.delay_steps);
        f.delay_triplet = boolean(x, "dtriplet", f.delay_triplet);
        f.delay_feedback = number(x, "dfb", f.delay_feedback);
        f.delay_pan = number(x, "dpan", f.delay_pan);
        f.delay_wet = number(x, "dwet", f.delay_wet);
        f.distortion_enabled = boolean(x, "distEnabled", f.distortion_enabled);
        f.distortion = number(x, "distAmt", f.distortion);
        f.distortion_shape = number(x, "distShape", f.distortion_shape);
        f.filter_enabled = boolean(x, "pcfEnabled", f.filter_enabled);
        f.filter_cutoff = number(x, "pcfFreq", f.filter_cutoff);
        f.filter_q = number(x, "pcfQ", f.filter_q);
        f.filter_amount = number(x, "pcfAmt", f.filter_amount);
        f.filter_decay = number(x, "pcfDec", f.filter_decay);
        f.filter_pattern = index(x, "pcfNum", f.filter_pattern);
        f.filter_mode = x
            .get("pcfMode")
            .and_then(Value::as_str)
            .unwrap_or(&f.filter_mode)
            .to_owned();
        f.compressor_enabled = boolean(x, "compEnabled", f.compressor_enabled);
        f.compressor_master = boolean(x, "mcomp", f.compressor_master);
        f.compressor_ratio = number(x, "compRatio", f.compressor_ratio);
        f.compressor_threshold = number(x, "compThres", f.compressor_threshold)
    }
    if let Some(send) = v.get("send") {
        for (i, key) in ["a", "b", "e8", "e9"].iter().enumerate() {
            if let Some(x) = send.get(key) {
                let c = &mut s.channels[i];
                c.delay = number(x, "l", c.delay);
                c.distortion = boolean(x, "d", c.distortion);
                c.filter = boolean(x, "p", c.filter);
                c.compressor = boolean(x, "c", c.compressor)
            }
        }
    }
    if let Some(a) = v.get("automation") {
        s.automation = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?
    }
    s.validate()?;
    Ok(s)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn example_songs() {
        for (b, bpm, bars) in [
            (
                include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").as_slice(),
                90.,
                564,
            ),
            (
                include_bytes!("../../../songs/rbs-examples/Plucked 303-01.rbs").as_slice(),
                128.,
                129,
            ),
            (
                include_bytes!("../../../songs/rbs-examples/ReBirthday.rbs").as_slice(),
                116.,
                180,
            ),
        ] {
            let s = import(b).unwrap();
            assert_eq!(s.bpm, bpm);
            assert_eq!(s.arrangement.len(), bars);
            assert_eq!(s.patterns.len(), 32);
            assert!(!s.automation.is_empty())
        }
    }
    #[test]
    fn malformed() {
        assert!(import(b"CAT ").is_err());
        let mut b = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        b[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(import(&b).is_err());
        assert!(import(&b[..100]).is_err());
    }
    #[test]
    fn legacy_rejects_missing_data() {
        assert!(import_legacy("{}").is_err())
    }
}
#[cfg(test)]
mod legacy_test {
    use super::*;
    use serde_json::json;
    #[test]
    fn legacy_snapshot() {
        let base = Song::default();
        let patterns: Vec<Value> = base
            .patterns
            .iter()
            .map(|p| {
                let mut a = serde_json::Map::new();
                a.insert("s1".into(), json!(p.notes[0]));
                a.insert("s2".into(), json!(p.notes[1]));
                for (k, ids) in [(0, DRUM_IDS), (1, DRUM_IDS_B)] {
                    let mut kit = serde_json::Map::new();
                    for (i, id) in ids.iter().enumerate() {
                        if k == 1 && *id == "AC" {
                            continue;
                        }
                        kit.insert(id.to_ascii_lowercase(), json!(p.drums[k][i]));
                    }
                    a.insert(
                        if k == 0 { "d808" } else { "d909" }.into(),
                        Value::Object(kit),
                    );
                }
                Value::Object(a)
            })
            .collect();
        let data = json!({"bpm":130,"start":1,"len":1,"slots":{"s1":0,"s2":1,"d808":2,"d909":3},"song":[{"s1":0,"s2":1,"d808":2,"d909":3}],"patterns":patterns,"t":{"a":{"res":10,"wave":"sawtooth","cutoff":5000}},"v":{"master":0.5},"send":{"a":{"l":0.7}},"automation":[{"tick":32,"track":1,"controller":3,"value":127}],"autoBase":{"t":{"a":{"res":10,"wave":"sawtooth","cutoff":1000}},"v":{"master":0.5},"send":{"a":{"l":0.7}},"drum":{"808":{"tune":{"bd":-3,"cy":3}}}}});
        let s = import_legacy(&data.to_string()).unwrap();
        assert_eq!(s.bpm, 130.);
        assert_eq!(s.synth[0].resonance, 0.5);
        assert_eq!(s.synth[0].cutoff, 1000.);
        assert_eq!(s.drums[0].voices[1].tone, 0.25);
        assert_eq!(s.drums[0].voices[9].tone, 0.75);
        assert_eq!(s.fx.master, 0.5);
        assert_eq!(s.channels[0].delay, 0.7);
        assert_eq!(s.patterns, base.patterns);
    }
}

#[cfg(test)]
mod regression_mapping_tests {
    use super::*;
    fn event(song: &mut Song, track: u8, controller: u8, value: u8) {
        apply_automation(
            song,
            &Automation {
                tick: 0,
                track,
                controller,
                value,
            },
        );
    }
    #[test]
    fn enable_and_mixer_mute_are_independent() {
        let mut song = Song::default();
        song.channels[0].mute = true;
        for track in 1..=4 {
            event(&mut song, track, 0, 0);
            assert!(!song.enabled[(track - 1) as usize]);
            event(&mut song, track, 0, 127);
            assert!(song.enabled[(track - 1) as usize]);
        }
        assert!(song.channels[0].mute);
        assert!(!song.channels[1].mute);
    }
    #[test]
    fn automation_tune_tone_and_feedback_endpoints() {
        let mut song = Song::default();
        event(&mut song, 1, 2, 0);
        assert_eq!(song.synth[0].tune, -12.);
        event(&mut song, 1, 2, 127);
        assert_eq!(song.synth[0].tune, 12.);
        for (control, id) in [(4, 1), (24, 9)] {
            let old = song.drums[0].voices[id].tune;
            event(&mut song, 3, control, 0);
            assert_eq!(song.drums[0].voices[id].tone, 0.);
            event(&mut song, 3, control, 127);
            assert_eq!(song.drums[0].voices[id].tone, 1.);
            assert_eq!(song.drums[0].voices[id].tune, old);
        }
        event(&mut song, 5, 3, 127);
        assert_eq!(song.fx.delay_feedback, 1.);
        assert!(song.validate().is_ok());
    }
    #[test]
    fn rbs_device_state_and_tone_bytes() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        for tag in [b"303 ", b"808 ", b"909 "] {
            let pos = raw.windows(4).position(|x| x == tag).unwrap();
            raw[pos + 8] = 0;
        }
        let p = raw.windows(4).position(|x| x == b"808 ").unwrap() + 8;
        raw[p + 4] = 127;
        raw[p + 24] = 0;
        let song = import(&raw).unwrap();
        assert_eq!(song.enabled, [false, true, false, false]);
        assert!(song.channels.iter().all(|c| !c.mute));
        assert_eq!(song.drums[0].voices[1].tone, 1.);
        assert_eq!(song.drums[0].voices[9].tone, 0.);
    }
    #[test]
    fn global_settings_loop_and_delay_modes() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        let p = raw.windows(4).position(|x| x == b"GLOB").unwrap() + 8;
        assert!(be(&raw, p - 4).unwrap() > 482);
        raw[p] = 0;
        raw[p + 1] = 0;
        raw[p + 6..p + 10].copy_from_slice(&768u32.to_be_bytes());
        raw[p + 10..p + 14].copy_from_slice(&2304u32.to_be_bytes());
        raw[p + 14] = 64;
        raw[p + 482] = 1;
        let d = raw.windows(4).position(|x| x == b"DELY").unwrap() + 8;
        raw[d + 2] = 0;
        let song = import(&raw).unwrap();
        assert!(!song.song_mode && !song.looping && song.vintage);
        assert_eq!(song.shuffle, 64. / 127. * 60.);
        assert_eq!((song.start, song.length), (1, 2));
        assert!(song.arrangement.len() >= 3);
        assert!(song.fx.delay_triplet);
        raw[d + 2] = 1;
        assert!(!import(&raw).unwrap().fx.delay_triplet);
        let track = raw.windows(4).enumerate().filter(|(_, x)| *x == b"TRAK").nth(5).unwrap().0 + 8;
        let count = be(&raw, track).unwrap();
        let mut pos = track + 4;
        let mut tick = 0;
        let mut found = false;
        for _ in 0..count {
            let mut delta = 0;
            loop {
                let byte = raw[pos];
                pos += 1;
                delta = delta * 128 + (byte & 127) as u32;
                if byte & 128 == 0 { break; }
            }
            tick += delta;
            if raw[pos] == 2 {
                raw[pos + 1] = 0;
                found = true;
                break;
            }
            pos += 2;
        }
        assert!(found);
        assert!(import(&raw).unwrap().automation.iter().any(|e| e.track == 5 && e.controller == 2 && e.tick == tick && e.value == 1));
        let mut song = Song::default();
        event(&mut song, 5, 2, 1);
        assert!(song.fx.delay_triplet);
        event(&mut song, 5, 2, 0);
        assert!(!song.fx.delay_triplet);
        raw[p + 10..p + 14].copy_from_slice(&2305u32.to_be_bytes());
        assert!(import(&raw).unwrap_err().contains("whole bars"));
    }
    #[test]
    fn rbs_selectors_convert_at_import_boundary() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        let p = raw.windows(4).position(|x| x == b"808 ").unwrap() + 8;
        for c in [11, 14, 17, 19, 21] { raw[p + c] = 0; }
        let song = import(&raw).unwrap();
        for i in [3, 4, 5, 6, 7] { assert!(song.drums[0].voices[i].variant); }
        let mut song = Song::default();
        event(&mut song, 3, 11, 0);
        assert!(!song.drums[0].voices[3].variant);
        event(&mut song, 3, 11, 1);
        assert!(song.drums[0].voices[3].variant);
        assert_eq!(rbs_control_value(5, 2, 0), 1);
        assert_eq!(rbs_control_value(5, 2, 1), 0);
        assert_eq!(rbs_control_value(3, 11, 0), 1);
        assert_eq!(rbs_control_value(3, 11, 1), 0);
    }
    #[test]
    fn rbs_filter_routing_uses_original_ids() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        let mixer = raw.windows(4).position(|x| x == b"MIXR").unwrap() + 8;
        let track = raw.windows(4).position(|x| x == b"TRAK").unwrap() + 8;
        let count = be(&raw, track).unwrap();
        let mut pos = track + 4;
        let mut tick = 0u32;
        let mut event_pos = None;
        for _ in 0..count {
            let mut delta = 0;
            loop {
                let byte = raw[pos];
                pos += 1;
                delta = delta * 128 + (byte & 127) as u32;
                if byte & 128 == 0 { break; }
            }
            tick += delta;
            if raw[pos] == 2 { event_pos = Some((pos + 1, tick)); break; }
            pos += 2;
        }
        let (value_pos, tick) = event_pos.unwrap();
        for (source, channel) in [(1, 0), (4, 3)] {
            raw[mixer + 2] = source;
            raw[value_pos] = source;
            let mut song = import(&raw).unwrap();
            assert_eq!(song.channels.each_ref().map(|c| c.filter), std::array::from_fn(|i| i == channel));
            let automation = song.automation.iter().find(|e| e.track == 0 && e.controller == 2 && e.tick == tick).unwrap().clone();
            assert_eq!(automation.value, source + 1);
            apply_automation(&mut song, &automation);
            assert_eq!(song.channels.each_ref().map(|c| c.filter), std::array::from_fn(|i| i == channel));
        }
        let mut song = Song::default();
        event(&mut song, 0, 2, 3);
        assert_eq!(song.channels.each_ref().map(|c| c.filter), [false, true, false, false]);
        assert_eq!(rbs_control_value(0, 2, 0), 0);
        assert_eq!(rbs_control_value(0, 2, 255), 255);
    }
    #[test]
    fn selected_pattern_shuffle_flag_is_imported() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        let glob = raw.windows(4).position(|x| x == b"GLOB").unwrap() + 8;
        raw[glob + 14] = 127;
        let device = raw.windows(4).position(|x| x == b"303 ").unwrap() + 8;
        let slot = raw[device + 1] as usize;
        raw[device + 9 + slot * 34] = 0;
        let song = import(&raw).unwrap();
        assert_eq!(song.shuffle, 60.);
        assert!(!song.shuffle_tracks[0]);
    }
    #[test]
    fn terminal_tick_fits_the_last_bar() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        let track = raw.windows(4).enumerate().filter(|(_, x)| *x == b"TRAK").nth(1).unwrap().0 + 8;
        let count = be(&raw, track).unwrap();
        let mut pos = track + 4;
        let mut tick = 0usize;
        let mut deltas = Vec::new();
        for _ in 0..count {
            let start = pos;
            let mut delta = 0usize;
            loop {
                let byte = raw[pos];
                pos += 1;
                delta = delta * 128 + (byte & 127) as usize;
                if byte & 128 == 0 { break; }
            }
            tick += delta;
            if pos - start == 2 { deltas.push((start, delta)); }
            pos += 2;
        }
        let mut remaining = 4096 * 32 - tick;
        let mut last = None;
        for (pos, delta) in deltas {
            let add = remaining.min(16382 - delta);
            let value = delta + add;
            raw[pos] = 128 | (value >> 7) as u8;
            raw[pos + 1] = (value & 127) as u8;
            remaining -= add;
            if add > 0 { last = Some((pos, value)); }
        }
        assert_eq!(remaining, 0);
        assert_eq!(import(&raw).unwrap().arrangement.len(), 4096);
        let (pos, value) = last.unwrap();
        raw[pos] = 128 | ((value + 1) >> 7) as u8;
        raw[pos + 1] = ((value + 1) & 127) as u8;
        assert!(import(&raw).is_err());
    }
    #[test]
    fn rejects_truncated_chunks_and_oversized_declared_header() {
        let original = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs");
        let mut raw = original.to_vec();
        raw[4..8].copy_from_slice(&(128u32 * 1024 * 1024).to_be_bytes());
        assert!(import(&raw).is_err());
        let mut raw = original.to_vec();
        raw[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(import(&raw).is_err());
        let mut raw = original.to_vec();
        raw.pop();
        let n = raw.len() as u32 - 8;
        raw[4..8].copy_from_slice(&n.to_be_bytes());
        assert!(import(&raw).is_err());
    }
}
