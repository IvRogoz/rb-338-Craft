use serde::{Deserialize, Serialize};

pub mod arrangement;
pub mod rbs;
pub const DRUM_IDS: [&str; 12] = [
    "AC", "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB", "CY", "OH", "CH",
];
pub const DRUM_IDS_B: [&str; 12] = [
    "AC", "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CH", "OH", "CC", "RC",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub note: u8,
    pub gate: bool,
    pub accent: bool,
    pub slide: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pattern {
    pub notes: [Vec<Note>; 2],
    pub drums: [Vec<Vec<u8>>; 2],
    pub lengths: [usize; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Synth {
    pub wave: String,
    pub tune: f32,
    pub cutoff: f32,
    pub resonance: f32,
    pub envelope: f32,
    pub decay: f32,
    pub accent: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Drum {
    pub level: f32,
    pub tune: f32,
    pub decay: f32,
    pub tone: f32,
    pub snappy: f32,
    pub attack: f32,
    pub variant: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrumKit {
    pub voices: Vec<Drum>,
    pub flam: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub delay: f32,
    pub distortion: bool,
    pub filter: bool,
    pub compressor: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Effects {
    pub master: f32,
    pub delay_enabled: bool,
    pub delay_steps: usize,
    pub delay_triplet: bool,
    pub delay_feedback: f32,
    pub delay_pan: f32,
    pub delay_wet: f32,
    pub distortion_enabled: bool,
    pub distortion: f32,
    pub distortion_shape: f32,
    pub filter_enabled: bool,
    pub filter_cutoff: f32,
    pub filter_q: f32,
    pub filter_amount: f32,
    pub filter_decay: f32,
    pub filter_pattern: usize,
    pub filter_mode: String,
    pub compressor_enabled: bool,
    pub compressor_master: bool,
    pub compressor_ratio: f32,
    pub compressor_threshold: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Automation {
    pub tick: u32,
    pub track: u8,
    pub controller: u8,
    pub value: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ControlState {
    pub synth: [Synth; 2],
    pub drums: [DrumKit; 2],
    pub channels: [Channel; 4],
    pub fx: Effects,
    pub enabled: [bool; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Song {
    pub version: u32,
    pub name: String,
    pub bpm: f32,
    pub shuffle: f32,
    #[serde(default = "all_true")]
    pub enabled: [bool; 4],
    #[serde(default = "all_true")]
    pub shuffle_tracks: [bool; 4],
    #[serde(default)]
    pub vintage: bool,
    pub looping: bool,
    pub song_mode: bool,
    pub start: usize,
    pub length: usize,
    pub slots: [usize; 4],
    pub patterns: Vec<Pattern>,
    pub arrangement: Vec<[usize; 4]>,
    pub synth: [Synth; 2],
    pub drums: [DrumKit; 2],
    pub channels: [Channel; 4],
    pub fx: Effects,
    pub automation: Vec<Automation>,
    #[serde(default)]
    pub automation_base: Option<ControlState>,
}
impl Default for Song {
    fn default() -> Self {
        let synth = Synth {
            wave: "saw".into(),
            tune: 0.,
            cutoff: 1800.,
            resonance: 0.35,
            envelope: 2400.,
            decay: 0.28,
            accent: 0.6,
        };
        let drum = Drum {
            level: 0.8,
            tune: 0.,
            decay: 1.,
            tone: 0.5,
            snappy: 0.5,
            attack: 0.5,
            variant: false,
        };
        let channel = Channel {
            level: 0.8,
            pan: 0.,
            mute: false,
            delay: 0.25,
            distortion: false,
            filter: false,
            compressor: false,
        };
        let patterns = (0..32)
            .map(|slot| {
                let notes = std::array::from_fn(|track| {
                    (0..16)
                        .map(|step| Note {
                            note: (if track == 0 { 36 } else { 31 })
                                + [0, 0, 12, 0, 3, 0, 10, 7, 0, 0, 0, 12, 10, 3, 5, 7][step]
                                + (slot % 3) as u8,
                            gate: step % 4 != 3,
                            accent: step % 4 == 0,
                            slide: step == 2 || step == 9,
                        })
                        .collect()
                });
                let drums = std::array::from_fn(|kit| {
                    let mut rows = vec![vec![0; 16]; 12];
                    for step in 0..16 {
                        rows[1][step] = u8::from(step % 4 == 0);
                        rows[2][step] = u8::from(step % 8 == 4);
                        rows[if kit == 0 { 11 } else { 8 }][step] = u8::from(step % 2 == 0);
                    }
                    rows
                });
                Pattern {
                    notes,
                    drums,
                    lengths: [16; 4],
                }
            })
            .collect();
        Self {
            version: 1,
            name: "Untitled".into(),
            bpm: 127.,
            shuffle: 0.,
            enabled: [true; 4],
            shuffle_tracks: [true; 4],
            vintage: false,
            looping: true,
            song_mode: false,
            start: 0,
            length: 2,
            slots: [0, 1, 2, 3],
            patterns,
            arrangement: vec![[0, 1, 2, 3], [4, 5, 6, 7]],
            synth: [synth.clone(), synth],
            drums: std::array::from_fn(|_| DrumKit {
                voices: vec![drum.clone(); 12],
                flam: 0.03,
            }),
            channels: std::array::from_fn(|_| channel.clone()),
            fx: Effects {
                master: 0.75,
                delay_enabled: true,
                delay_steps: 3,
                delay_triplet: false,
                delay_feedback: 0.3,
                delay_pan: 0.,
                delay_wet: 0.4,
                distortion_enabled: true,
                distortion: 0.2,
                distortion_shape: 0.3,
                filter_enabled: true,
                filter_cutoff: 1600.,
                filter_q: 1.,
                filter_amount: 0.4,
                filter_decay: 0.2,
                filter_pattern: 0,
                filter_mode: "LP".into(),
                compressor_enabled: true,
                compressor_master: true,
                compressor_ratio: 4.,
                compressor_threshold: -12.,
            },
            automation: vec![],
            automation_base: None,
        }
    }
}
impl Song {
    pub fn control_state(&self) -> ControlState {
        ControlState {
            synth: self.synth.clone(),
            drums: self.drums.clone(),
            channels: self.channels.clone(),
            fx: self.fx.clone(),
            enabled: self.enabled,
        }
    }
    pub fn restore_control_base(&mut self) {
        if let Some(base) = &self.automation_base {
            // Mixer mute and master level are explicitly not song automation.
            let mute = self.channels.each_ref().map(|c| c.mute);
            let master = self.fx.master;
            let wet = self.fx.delay_wet;
            self.synth = base.synth.clone();
            self.drums = base.drums.clone();
            self.channels = base.channels.clone();
            self.fx = base.fx.clone();
            self.enabled = base.enabled;
            for (i, value) in mute.into_iter().enumerate() {
                self.channels[i].mute = value;
            }
            self.fx.master = master;
            self.fx.delay_wet = wet;
        }
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() > 16 * 1024 * 1024 {
            return Err("Song exceeds 16 MB".into());
        }
        let song: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        song.validate()?;
        Ok(song)
    }
    pub fn validate(&self) -> Result<(), String> {
        let fail = || Err("Invalid song data".to_string());
        if self.version != 1
            || self.patterns.len() != 32
            || self.arrangement.len() > 4096
            || self.slots.iter().any(|i| *i >= 32)
            || self.arrangement.iter().flatten().any(|i| *i >= 32)
        {
            return fail();
        }
        if !finite(self.bpm, 20., 500.)
            || !finite(self.shuffle, 0., 60.)
            || self.length == 0
            || self.length > 4096
            || self.start >= self.arrangement.len().max(1)
        {
            return fail();
        }
        for p in &self.patterns {
            if p.lengths.iter().any(|n| !(1..=16).contains(n))
                || p.notes
                    .iter()
                    .any(|n| n.len() != 16 || n.iter().any(|s| s.note > 127))
                || p.drums.iter().any(|d| {
                    d.len() != 12 || d.iter().any(|r| r.len() != 16 || r.iter().any(|v| *v > 3))
                })
            {
                return fail();
            }
        }
        for s in &self.synth {
            if !["saw", "square", "sine", "triangle"].contains(&s.wave.as_str())
                || !finite(s.tune, -24., 24.)
                || !finite(s.cutoff, 20., 20000.)
                || !finite(s.resonance, 0., 1.)
                || !finite(s.envelope, 0., 12000.)
                || !finite(s.decay, 0.01, 4.)
                || !finite(s.accent, 0., 1.)
            {
                return fail();
            }
        }
        for k in &self.drums {
            if k.voices.len() != 12 || !finite(k.flam, 0.005, 0.2) {
                return fail();
            }
            for d in &k.voices {
                if !finite(d.level, 0., 1.5)
                    || !finite(d.tune, -24., 24.)
                    || !finite(d.decay, 0.1, 4.)
                    || !finite(d.tone, 0., 1.)
                    || !finite(d.snappy, 0., 1.)
                    || !finite(d.attack, 0., 1.)
                {
                    return fail();
                }
            }
        }
        for c in &self.channels {
            if !finite(c.level, 0., 1.5) || !finite(c.pan, -1., 1.) || !finite(c.delay, 0., 1.) {
                return fail();
            }
        }
        let f = &self.fx;
        if !finite(f.master, 0., 1.5)
            || !(1..=32).contains(&f.delay_steps)
            || !finite(f.delay_feedback, 0., 1.)
            || !finite(f.delay_pan, -1., 1.)
            || !finite(f.delay_wet, 0., 1.)
            || !finite(f.distortion, 0., 1.)
            || !finite(f.distortion_shape, 0., 1.)
            || !finite(f.filter_cutoff, 20., 20000.)
            || !finite(f.filter_q, 0.1, 15.)
            || !finite(f.filter_amount, 0., 1.)
            || !finite(f.filter_decay, 0.01, 10.)
            || f.filter_pattern > 54
            || !["LP", "BP"].contains(&f.filter_mode.as_str())
            || !finite(f.compressor_ratio, 1., 20.)
            || !finite(f.compressor_threshold, -60., 0.)
        {
            return fail();
        }
        if self.automation.len() > 1_000_000
            || self.automation.iter().any(|e| e.tick > 4096 * 32)
            || self.automation.windows(2).any(|w| w[0].tick > w[1].tick)
        {
            return fail();
        }
        if let Some(base) = &self.automation_base {
            let mut check = self.clone();
            check.automation_base = None;
            check.synth = base.synth.clone();
            check.drums = base.drums.clone();
            check.channels = base.channels.clone();
            check.fx = base.fx.clone();
            check.enabled = base.enabled;
            check.validate()?;
        }
        Ok(())
    }
}
fn finite(v: f32, min: f32, max: f32) -> bool {
    v.is_finite() && v >= min && v <= max
}
fn all_true() -> [bool; 4] {
    [true; 4]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let s = Song::default();
        s.validate().unwrap();
        assert_eq!(
            s,
            Song::from_json(&serde_json::to_string(&s).unwrap()).unwrap()
        );
    }
    #[test]
    fn reject_bad_bank() {
        let mut s = Song::default();
        s.patterns[0].drums[1].clear();
        assert!(s.validate().is_err());
    }
}

#[cfg(test)]
mod baseline_regression_tests {
    use super::*;
    #[test]
    fn invalid_automation_baseline_is_rejected() {
        let mut s = Song::default();
        let mut base = s.control_state();
        base.synth[0].resonance = 2.;
        s.automation_base = Some(base);
        assert!(s.validate().is_err());
        assert!(Song::from_json(&serde_json::to_string(&s).unwrap()).is_err());
        let mut base = s.control_state();
        base.drums[0].voices.clear();
        s.automation_base = Some(base);
        assert!(s.validate().is_err());
        let mut base = s.control_state();
        base.fx.delay_feedback = 1.;
        s.automation_base = Some(base);
        assert!(s.validate().is_ok());
    }
}
