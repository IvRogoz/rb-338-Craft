//! Sample-free acid voice: analytic oscillator, pre-pole, resonant pole pair and DC blocker.
use rb_model::{Note, Synth};
#[cfg(test)]
use std::f32::consts::PI;
use std::f64::consts::TAU;

pub struct AcidVoice {
    sr: f32,
    phase: f64,
    pitch: f32,
    target: f32,
    glide: bool,
    pending_slide: bool,
    filter_env: f32,
    amp: f32,
    gate_samples: f32,
    accented: bool,
    vintage: bool,
    glide_factor: f32,
    decay_setting: f32,
    decay_factor: f32,
    accent_env: f32,
    coefficients: [f64; 4],
    coefficient_steps: [f64; 4],
    coefficient_left: u16,
    retrigger: bool,
    cutoff_state: [f64; 2],
    cutoff_initialized: bool,
    smoothing_left: u16,
    pre: f64,
    body: [f64; 2],
    dc_input: f64,
    dc: f64,
}

// A standard damped pole pair, preceded by a one-pole low-pass. No coefficient ROM.
fn poles(position: f64, resonance: f64, sr: f64) -> [f64; 4] {
    let position = (position * 1023.).floor() / 1023.;
    let hz =
        (44100. * 0.4 / 1602. * 1602.0_f64.powf(position) * (0.6 + 0.4 * resonance)).min(sr * 0.4);
    let blend = ((position * 1023.).powi(2) / 512000. - 0.3).clamp(0., 1.);
    let q =
        (0.5 + 7. / 6. * resonance) * (1. - blend) + 0.52 * 1.78_f64.powf(7. * resonance) * blend;
    let angle = TAU * hz / sr;
    let radius = (-angle / (2. * q)).exp();
    let a = 2. * radius * angle.cos();
    let b = radius * radius;
    let pre = (-angle * (0.15 + 0.85 * resonance)).exp();
    let gain = (1. - a + b) * (1. - pre) / (0.8 * (1. + if q > 4. { 3. } else { q }));
    [a, b, gain, pre]
}

impl AcidVoice {
    pub fn new(sr: f32) -> Self {
        let sr = if sr.is_finite() {
            sr.clamp(8000., 192000.)
        } else {
            44100.
        };
        Self {
            sr,
            phase: 0.5,
            pitch: 36.,
            target: 36.,
            glide: false,
            pending_slide: false,
            filter_env: 0.,
            amp: 0.,
            gate_samples: 0.,
            accented: false,
            vintage: false,
            glide_factor: 1. - (-1. / (0.0167 * sr)).exp(),
            decay_setting: f32::NAN,
            decay_factor: 0.,
            accent_env: 0.,
            coefficients: [0.; 4],
            coefficient_steps: [0.; 4],
            coefficient_left: 0,
            retrigger: false,
            cutoff_state: [0.; 2],
            cutoff_initialized: false,
            smoothing_left: 0,
            pre: 0.,
            body: [0.; 2],
            dc_input: 0.,
            dc: 0.,
        }
    }
    pub fn clear(&mut self) {
        *self = Self::new(self.sr);
    }
    pub fn note(&mut self, note: &Note, _cfg: &Synth, duration: f32, vintage: bool) {
        let tied = self.pending_slide;
        self.vintage = vintage;
        self.accented = note.accent;
        self.decay_setting = f32::NAN;
        if tied {
            self.target = note.note as f32;
            self.glide = true;
        } else {
            self.pitch = note.note as f32;
            self.target = self.pitch;
            self.glide = false;
            if note.gate {
                self.filter_env = 1.;
                self.retrigger = true;
            }
        }
        self.gate_samples = if note.gate && note.slide {
            f32::INFINITY
        } else if note.gate || tied {
            (duration.max(0.001) * 0.5 * self.sr).max(1.)
        } else {
            0.
        };
        if !note.gate && note.slide {
            self.pitch = note.note as f32;
            self.target = self.pitch;
        }
        self.pending_slide = note.slide;
        self.coefficient_left = 0;
    }
    pub fn sample(&mut self, cfg: &Synth) -> f32 {
        if self.glide {
            self.pitch += (self.target - self.pitch) * self.glide_factor;
            if (self.target - self.pitch).abs() < 0.0001 {
                self.pitch = self.target;
                self.glide = false;
            }
        }
        let gated = self.gate_samples > 0.;
        let smoothing = if self.vintage {
            1. - (-1. / (0.00086 * self.sr)).exp()
        } else if gated {
            1.
        } else {
            1. - (-1. / (0.0223 * self.sr)).exp()
        };
        self.amp += (if gated { 1. } else { 0. } - self.amp) * smoothing;
        if self.gate_samples.is_finite() && gated {
            self.gate_samples -= 1.;
        }
        let decay = if self.accented { -1. } else { cfg.decay };
        if decay != self.decay_setting {
            let amount = ((cfg.decay - 0.05) / 0.75).clamp(0., 1.);
            let rate = if self.accented {
                13.67
            } else {
                44100. * (0.00001 + 0.0003 * (1. - amount).powi(2))
            };
            self.decay_factor = (-rate / self.sr).exp();
            self.decay_setting = decay;
        }
        self.filter_env *= self.decay_factor;
        let resonance = cfg.resonance.clamp(0., 1.);
        let accent_follow = 1. - ((0.98901 + resonance * 0.00999).ln() * (44100. / self.sr)).exp();
        let accent_target = if self.accented {
            self.filter_env * 3. * cfg.accent
        } else {
            0.
        };
        self.accent_env += (accent_target - self.accent_env) * accent_follow;
        let silent = !gated && self.amp < 1e-7;
        if silent {
            self.amp = 0.;
        }
        if self.smoothing_left == 0 {
            let cutoff = ((cfg.cutoff - 100.) / 5900.).clamp(0., 1.) as f64;
            let target = 0.46 + cutoff * 0.29;
            if !self.cutoff_initialized {
                self.cutoff_state = [target; 2];
                self.cutoff_initialized = true;
            }
            // The cutoff control has a slow underdamped response, independent of note envelopes.
            let w: f64 = 0.0628;
            let radius = (-w * 0.5).exp();
            let a = 2. * radius * w.cos();
            let b = radius * radius;
            let cutoff =
                a * self.cutoff_state[0] - b * self.cutoff_state[1] + (1. - a + b) * target;
            self.cutoff_state = [cutoff, self.cutoff_state[0]];
            self.smoothing_left = (64. * self.sr / 44100.).round().max(1.) as u16;
        }
        self.smoothing_left -= 1;
        if self.coefficient_left == 0 {
            let count = ((if self.vintage { 4. } else { 64. }) * self.sr / 44100.)
                .round()
                .max(1.) as u16;
            let cutoff = self.cutoff_state[0];
            let envmod = (cfg.envelope / 5000.).clamp(0., 1.) as f64;
            let envelope = self.filter_env as f64 * (self.decay_factor as f64).powi(count as i32);
            let accent = accent_target as f64
                + (self.accent_env - accent_target) as f64
                    * (1. - accent_follow as f64).powi(count as i32 - 1);
            let amp_target = if gated { 1. } else { 0. };
            let amp = amp_target
                + (self.amp as f64 - amp_target) * (1. - smoothing as f64).powi(count as i32 - 1);
            let position = (cutoff
                + (envelope - 0.4) * (0.08 + 0.38 * envmod)
                + accent * (0.101667 - resonance as f64 / 15.))
                .clamp(0., 1.);
            let row = resonance as f64 * if self.vintage { 7.999 } else { 7. };
            let low = row.floor().min(7.);
            let mut next = poles(position, low / 7., self.sr as f64);
            if !self.vintage {
                let high = poles(position, (low + 1.).min(7.) / 7., self.sr as f64);
                // Modern sound blends resonance rows; Vintage keeps discrete settings.
                let fraction = (row - low).clamp(0., 1.);
                for i in 0..4 {
                    next[i] += (high[i] - next[i]) * fraction;
                }
            }
            next[2] *= amp * (1. + accent);
            if self.retrigger {
                self.coefficients = next;
                self.coefficients[2] = 0.;
                self.retrigger = false;
            }
            if self.vintage {
                self.coefficients = next;
                self.coefficient_steps = [0.; 4];
            } else {
                for i in 0..4 {
                    self.coefficient_steps[i] = (next[i] - self.coefficients[i]) / count as f64;
                }
            }
            self.coefficient_left = count;
        }
        let frequency = (440.
            * 2.0_f64.powf((self.pitch as f64 + cfg.tune.round() as f64 - 69.) / 12.))
        .clamp(1., self.sr as f64 * 0.43);
        self.phase = (self.phase + frequency / self.sr as f64).fract();
        let oscillator = match cfg.wave.as_str() {
            "square" => {
                if self.phase < 0.522888 {
                    -0.549316
                } else {
                    0.549316
                }
            }
            "sine" => (TAU * self.phase).sin(),
            "triangle" => 1. - 4. * (self.phase - 0.5).abs(),
            _ => 2. * self.phase - 1.,
        };
        let [a, b, g, p] = self.coefficients;
        self.pre = oscillator + p * self.pre;
        let y = self.pre * g + a * self.body[0] - b * self.body[1];
        self.dc = y - self.dc_input + (-TAU * 100. / self.sr as f64).exp() * self.dc;
        self.dc_input = y;
        self.body = [y, self.body[0]];
        for i in 0..4 {
            self.coefficients[i] += self.coefficient_steps[i];
        }
        self.coefficient_left -= 1;
        // The oscillator and input pole keep running between notes, including during silence.
        if silent {
            0.
        } else {
            self.dc as f32 / 2.18
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silent_oscillator_preserves_phase_and_input_pole_history() {
        let c = rb_model::Song::default().synth[0].clone();
        let mut voice = AcidVoice::new(44100.);
        let phase = voice.phase;
        for _ in 0..137 {
            assert_eq!(voice.sample(&c), 0.);
        }
        assert_ne!(voice.phase, phase);
        assert!(voice.pre.is_finite() && voice.pre.abs() > 1e-5);
        let phase = voice.phase;
        voice.note(&note(48, true, false, false), &c, 0.125, false);
        assert_eq!(voice.phase, phase);
    }
    fn note(p: u8, gate: bool, accent: bool, slide: bool) -> Note {
        Note {
            note: p,
            gate,
            accent,
            slide,
        }
    }
    #[test]
    fn ties_and_rest_glide_keep_state() {
        let c = Synth {
            wave: "saw".into(),
            tune: 0.,
            cutoff: 900.,
            resonance: 0.7,
            envelope: 2000.,
            decay: 1.,
            accent: 0.5,
        };
        let mut v = AcidVoice::new(48000.);
        v.note(&note(48, true, false, true), &c, 0.2, false);
        for _ in 0..1000 {
            v.sample(&c);
        }
        let amp = v.amp;
        let env = v.filter_env;
        v.note(&note(48, true, false, false), &c, 0.2, false);
        assert_eq!(v.amp, amp);
        assert_eq!(v.filter_env, env);
        v.note(&note(40, false, false, true), &c, 0.2, false);
        v.note(&note(52, true, false, false), &c, 0.2, false);
        assert_eq!(v.pitch, 40.);
        v.sample(&c);
        assert!(v.pitch > 40. && v.pitch < 52.);
    }
    #[test]
    fn accented_decay_and_live_tune() {
        let c = rb_model::Song::default().synth[0].clone();
        let mut a = AcidVoice::new(48000.);
        let mut b = AcidVoice::new(48000.);
        a.note(&note(48, true, true, false), &c, 1.0, false);
        b.note(&note(48, true, false, false), &c, 1.0, false);
        let mut slow = c.clone();
        slow.decay = 1.0;
        for _ in 0..9600 {
            a.sample(&slow);
            b.sample(&slow);
        }
        assert!(a.filter_env < b.filter_env);
        let phase = b.phase;
        slow.tune = 12.0;
        b.sample(&slow);
        assert!((b.phase - phase).abs() > 0.0);
    }
    #[test]
    fn sweep_is_finite_at_audio_rates() {
        for sr in [44100., 48000., 96000.] {
            let mut v = AcidVoice::new(sr);
            let mut c = rb_model::Song::default().synth[0].clone();
            c.cutoff = 20000.;
            c.resonance = 1.;
            c.envelope = 12000.;
            v.note(&note(100, true, true, false), &c, 1.0, false);
            for _ in 0..sr as usize {
                assert!(v.sample(&c).is_finite());
            }
        }
    }
    #[test]
    fn resonance_and_cutoff_change_waveform() {
        let mut c = rb_model::Song::default().synth[0].clone();
        c.envelope = 0.;
        let mut a = AcidVoice::new(48000.);
        let mut b = AcidVoice::new(48000.);
        a.note(&note(48, true, false, false), &c, 0.5, false);
        b.note(&note(48, true, false, false), &c, 0.5, false);
        let mut d = c.clone();
        d.resonance = 1.;
        d.cutoff = 500.;
        let x: Vec<_> = (0..4096).map(|_| a.sample(&c)).collect();
        let y: Vec<_> = (0..4096).map(|_| b.sample(&d)).collect();
        assert_ne!(x, y);
    }
    #[test]
    fn square_retains_even_harmonics() {
        let mut c = rb_model::Song::default().synth[0].clone();
        c.wave = "square".into();
        c.cutoff = 20000.;
        c.resonance = 0.;
        c.envelope = 0.;
        let mut v = AcidVoice::new(48000.);
        v.note(&note(45, true, false, true), &c, 1., false);
        for _ in 0..4800 {
            v.sample(&c);
        }
        let x: Vec<_> = (0..4800).map(|_| v.sample(&c)).collect();
        let magnitude = |hz: f32| {
            let (mut re, mut im) = (0., 0.);
            for (i, sample) in x.iter().enumerate() {
                let phase = 2. * PI * hz * i as f32 / 48000.;
                re += sample * phase.cos();
                im += sample * phase.sin();
            }
            (re * re + im * im).sqrt()
        };
        let ratio = magnitude(220.) / magnitude(110.);
        assert!(
            (0.05..0.2).contains(&ratio),
            "second harmonic ratio {ratio}"
        );
    }
    #[test]
    fn output_pitch_and_resonant_spectrum() {
        fn render(res: f32) -> Vec<f32> {
            let mut c = rb_model::Song::default().synth[0].clone();
            c.wave = "saw".into();
            c.cutoff = 1046.5;
            c.envelope = 0.;
            c.resonance = res;
            let mut v = AcidVoice::new(48000.);
            v.note(&note(48, true, false, true), &c, 1., false);
            (0..24000).map(|_| v.sample(&c)).collect()
        }
        let lo = render(0.);
        let hi = render(0.9);
        let window = &lo[12000..24000];
        let crosses = window
            .windows(2)
            .filter(|w| w[0] <= 0. && w[1] > 0.)
            .count();
        assert!((30..=35).contains(&crosses), "zero crossings: {crosses}");
        fn magnitude(x: &[f32], hz: f32) -> f32 {
            let (mut re, mut im) = (0., 0.);
            for (i, v) in x.iter().enumerate() {
                let a = 2. * PI * hz * i as f32 / 48000.;
                re += v * a.cos();
                im += v * a.sin();
            }
            (re * re + im * im).sqrt()
        }
        let a = magnitude(&lo[12000..], 392.438) / magnitude(&lo[12000..], 130.813);
        let b = magnitude(&hi[12000..], 392.438) / magnitude(&hi[12000..], 130.813);
        assert!(b > a * 2., "normalized resonance peak {a} {b}");
        let mut wav = Vec::new();
        let data: Vec<u8> = hi
            .iter()
            .flat_map(|x| ((*x * 32767.0) as i16).to_le_bytes())
            .collect();
        wav.extend(b"RIFF");
        wav.extend((36 + data.len() as u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(48000u32.to_le_bytes());
        wav.extend(96000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((data.len() as u32).to_le_bytes());
        wav.extend(data);
        std::fs::write(std::env::temp_dir().join("rb338-acid-test.wav"), wav).unwrap();
    }

    #[test]
    fn time_and_pitch_consistent_across_sample_rates() {
        for sr in [44100., 48000., 96000.] {
            let mut c = rb_model::Song::default().synth[0].clone();
            c.decay = 0.05 + 0.75 * 64. / 127.;
            let mut v = AcidVoice::new(sr);
            v.note(&note(69, true, false, true), &c, 1., false);
            for _ in 0..(sr * 0.2) as usize {
                v.sample(&c);
            }
            assert!((v.filter_env - 0.477).abs() < 0.003);
            assert!((v.pitch - 69.).abs() < 0.001);
            assert!((v.amp - 1.).abs() < 0.001);
        }
    }
    #[test]
    fn accent_zero_and_vintage_timing() {
        let mut c = rb_model::Song::default().synth[0].clone();
        c.accent = 0.0;
        c.decay = 1.0;
        let mut normal = AcidVoice::new(48000.0);
        let mut accent = AcidVoice::new(48000.0);
        let mut vintage = AcidVoice::new(48000.0);
        normal.note(&note(48, true, false, false), &c, 1.0, false);
        accent.note(&note(48, true, true, false), &c, 1.0, false);
        vintage.note(&note(48, true, false, false), &c, 1.0, true);
        assert_eq!(accent.gate_samples, normal.gate_samples);
        for _ in 0..4800 {
            normal.sample(&c);
            accent.sample(&c);
            vintage.sample(&c);
        }
        assert!((normal.amp - accent.amp).abs() < 1e-6);
        assert!(accent.filter_env < normal.filter_env);
        assert!((vintage.filter_env - normal.filter_env).abs() < 1e-6);
        for _ in 0..48000 {
            normal.sample(&c);
            accent.sample(&c);
        }
        assert_eq!(normal.sample(&c), 0.0);
        assert_eq!(accent.sample(&c), 0.0);
    }
    #[test]
    fn vintage_has_fast_release_without_shortening_filter_decay() {
        let c = rb_model::Song::default().synth[0].clone();
        let mut modern = AcidVoice::new(48000.);
        let mut vintage = AcidVoice::new(48000.);
        modern.note(&note(48, true, false, false), &c, 0.1, false);
        vintage.note(&note(48, true, false, false), &c, 0.1, true);
        for _ in 0..3600 {
            modern.sample(&c);
            vintage.sample(&c);
        }
        assert!(modern.amp > vintage.amp * 10.0);
        assert!((modern.filter_env - vintage.filter_env).abs() < 1e-6);
    }
    #[test]
    fn release_render_cost() {
        let mut c = rb_model::Song::default().synth[0].clone();
        c.resonance = 0.95;
        c.envelope = 5000.0;
        let mut voices = [AcidVoice::new(48000.0), AcidVoice::new(48000.0)];
        for v in &mut voices {
            v.note(&note(48, true, false, true), &c, 10.0, false);
        }
        let start = std::time::Instant::now();
        let mut sum = 0.0;
        for _ in 0..480000 {
            for v in &mut voices {
                sum += v.sample(&c).abs();
            }
        }
        let elapsed = start.elapsed();
        eprintln!(
            "two acid voices: 10 seconds audio in {:?}; checksum {}",
            elapsed, sum
        );
        assert!(sum.is_finite() && sum > 0.0);
    }
}
