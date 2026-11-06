//! Sample-free acid voice. The exponential resonance control curve and the
//! 200 ms accent decay follow Robin Schmidt's MIT-licensed Open303 reference;
//! this is an independent digital filter, not its circuit or filter solver.
use rb_model::{Note, Synth};
use std::f32::consts::PI;

const OS: usize = 4;
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

pub struct AcidVoice {
    sr: f32,
    phase: f32,
    pitch: f32,
    target: f32,
    glide: bool,
    pending_slide: bool,
    filter_env: f32,
    amp: f32,
    gate_samples: f32,
    accented: bool,
    vintage: bool,
    attack: f32,
    release: f32,
    glide_factor: f32,
    hp: f32,
    aa: f32,
    cached_pitch: f32,
    frequency: f32,
    decay_setting: f32,
    decay_factor: f32,
    coeff_count: u8,
    coeff_key: [f32; 4],
    g: f32,
    feedback: f32,
    g_step: f32,
    feedback_step: f32,
    stages: [f32; 4],
    feedback_low: f32,
    decimate: [f32; 3],
    last: f32,
}
impl AcidVoice {
    pub fn new(sr: f32) -> Self {
        let sr = sr.clamp(8000.0, 192000.0);
        let osr = sr * OS as f32;
        Self {
            sr,
            phase: 0.0,
            pitch: 36.0,
            target: 36.0,
            glide: false,
            pending_slide: false,
            filter_env: 0.0,
            amp: 0.0,
            gate_samples: 0.0,
            accented: false,
            vintage: false,
            attack: 1.0 - (-1.0 / (0.003 * sr)).exp(),
            release: 1.0 - (-1.0 / (0.006 * sr)).exp(),
            glide_factor: 1.0 - (-1.0 / (0.012 * sr)).exp(),
            hp: 1.0 - (-2.0 * PI * 150.0 / osr).exp(),
            aa: 1.0 - (-2.0 * PI * (sr * 0.38) / osr).exp(),
            cached_pitch: f32::NAN,
            frequency: 0.0,
            decay_setting: f32::NAN,
            decay_factor: 0.0,
            coeff_count: 0,
            coeff_key: [f32::NAN; 4],
            g: 0.0,
            feedback: 0.0,
            g_step: 0.0,
            feedback_step: 0.0,
            stages: [0.0; 4],
            feedback_low: 0.0,
            decimate: [0.0; 3],
            last: 0.0,
        }
    }
    pub fn clear(&mut self) {
        *self = Self::new(self.sr);
    }
    pub fn note(&mut self, n: &Note, _cfg: &Synth, duration: f32, vintage: bool) {
        let pitch = n.note as f32;
        if !n.gate {
            self.gate_samples = 0.0;
            if n.slide {
                self.pitch = pitch;
                self.target = pitch;
            }
            self.pending_slide = n.slide;
            return;
        }
        let tied = self.pending_slide;
        if tied {
            self.target = pitch;
            self.glide = true;
        } else {
            self.pitch = pitch;
            self.target = pitch;
            self.glide = false;
            self.filter_env = 1.0;
        }
        self.accented = n.accent;
        self.vintage = vintage;
        self.coeff_count = 0;
        self.pending_slide = n.slide;
        self.gate_samples = if n.slide {
            f32::INFINITY
        } else {
            (duration.max(0.001) * (if n.accent { 0.4 } else { 0.5 }) * self.sr).max(1.0)
        };
    }
    pub fn sample(&mut self, cfg: &Synth) -> f32 {
        let sr = self.sr;
        let osr = sr * OS as f32;
        if self.glide {
            self.pitch += (self.target - self.pitch) * self.glide_factor;
            if (self.target - self.pitch).abs() < 0.0001 {
                self.pitch = self.target;
                self.glide = false;
            }
        }
        let pitch = self.pitch + cfg.tune;
        if pitch != self.cached_pitch {
            self.frequency = (440.0 * 2.0_f32.powf((pitch - 69.0) / 12.0)).clamp(1.0, sr * 0.43);
            self.cached_pitch = pitch;
        }
        let gated = self.gate_samples > 0.0;
        let level = 0.72 + if self.accented { 0.8 * cfg.accent } else { 0.0 };
        self.amp += ((if gated { level } else { 0.0 }) - self.amp)
            * if gated { self.attack } else { self.release };
        if self.gate_samples.is_finite() && self.gate_samples > 0.0 {
            self.gate_samples -= 1.0;
        }
        // The vintage shortening ratio is a design choice, not a measurement.
        let decay = if self.accented {
            0.2
        } else {
            cfg.decay.max(0.01) * if self.vintage { 0.5 } else { 1.0 }
        };
        if decay != self.decay_setting {
            self.decay_factor = (-1.0 / (decay * sr)).exp();
            self.decay_setting = decay;
        }
        let env = self.filter_env;
        self.filter_env *= self.decay_factor;
        if !gated && self.amp < 1e-7 {
            self.amp = 0.0;
            self.stages = [0.0; 4];
            self.feedback_low = 0.0;
            self.decimate = [0.0; 3];
            self.last = 0.0;
            return 0.0;
        }
        let key = [
            cfg.cutoff,
            cfg.resonance,
            cfg.envelope,
            if self.accented { cfg.accent } else { 0.0 },
        ];
        if self.coeff_count == 0 || key != self.coeff_key {
            self.coeff_key = key;
            self.coeff_count = 16;
            let modulation = (cfg.envelope / 2400.0).clamp(0.0, 5.0);
            let cutoff = (cfg.cutoff
                * 2.0_f32.powf(
                    env * modulation
                        * (1.0
                            + if self.accented {
                                0.65 * cfg.accent
                            } else {
                                0.0
                            }),
                ))
            .clamp(25.0, sr * 0.42);
            let r = cfg.resonance.clamp(0.0, 1.0);
            let resonance = (1.0 - (-3.0 * r).exp()) / (1.0 - (-3.0_f32).exp());
            // Frequency-compensated coefficients adapted from Open303 (MIT).
            let wc = 2.0 * PI * cutoff / osr;
            let (sn, cs) = wc.sin_cos();
            let t = (0.25 * (wc - PI)).tan();
            let a_full = t / (sn - cs * t);
            let a0 = -(-wc).exp();
            let a = resonance * a_full + (1.0 - resonance) * a0;
            let g = (1.0 + a).clamp(0.0, 0.99);
            let gsq = g * g / (1.0 + a * a + 2.0 * a * cs).max(1e-9);
            let feedback = (resonance / (gsq * gsq).max(1e-9)).min(3.9);
            self.g_step = (g - self.g) / 16.0;
            self.feedback_step = (feedback - self.feedback) / 16.0;
        }
        self.coeff_count -= 1;
        self.g += self.g_step;
        self.feedback += self.feedback_step;
        let (g, feedback, hp, aa) = (self.g, self.feedback, self.hp, self.aa);
        for _ in 0..OS {
            let dt = self.frequency / osr;
            self.phase = (self.phase + dt).fract();
            let saw = 2.0 * self.phase - 1.0 - blep(self.phase, dt);
            let wave = match cfg.wave.as_str() {
                "square" => {
                    let square = if self.phase < 0.5 { 1.0 } else { -1.0 };
                    (square + blep(self.phase, dt) - blep((self.phase + 0.5).fract(), dt)) * 0.8
                }
                "sine" => (2.0 * PI * self.phase).sin(),
                "triangle" => 1.0 - 4.0 * (self.phase - 0.5).abs(),
                _ => saw,
            };
            // Four-pole resonant digital low-pass with soft-limited feedback;
            // coefficient, feedback and saturation are design choices, not a circuit simulation.
            self.feedback_low += hp * (self.stages[3] - self.feedback_low);
            let mut x = (wave * 1.15 - feedback * (self.stages[3] - self.feedback_low)).tanh();
            for stage in &mut self.stages {
                *stage += g * (x - *stage);
                x = *stage;
            }
            x = self.stages[3];
            for stage in &mut self.decimate {
                *stage += aa * (x - *stage);
                x = *stage;
            }
            self.last = x;
        }
        self.last * self.amp * 0.35
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let a = magnitude(&lo[12000..], 1046.5);
        let b = magnitude(&hi[12000..], 1046.5);
        assert!(b > a, "resonance peak {a} {b}");
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
            c.decay = 1.;
            let mut v = AcidVoice::new(sr);
            v.note(&note(69, true, false, true), &c, 1., false);
            for _ in 0..(sr * 0.2) as usize {
                v.sample(&c);
            }
            assert!((v.filter_env - (-0.2_f32).exp()).abs() < 0.002);
            assert!((v.pitch - 69.).abs() < 0.001);
            assert!(v.amp > 0.65 && v.amp < 0.8);
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
        assert!(accent.gate_samples < normal.gate_samples);
        for _ in 0..4800 {
            normal.sample(&c);
            accent.sample(&c);
            vintage.sample(&c);
        }
        assert!((normal.amp - accent.amp).abs() < 1e-6);
        assert!(accent.filter_env < normal.filter_env);
        assert!(vintage.filter_env < normal.filter_env);
        for _ in 0..48000 {
            normal.sample(&c);
            accent.sample(&c);
        }
        assert_eq!(normal.sample(&c), 0.0);
        assert_eq!(accent.sample(&c), 0.0);
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
