use rb_model::Song;
use std::f32::consts::PI;

#[derive(Clone, Copy, Default)]
struct Filter {
    ic1: f32,
    ic2: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}
impl Filter {
    fn configure(&mut self, freq: f32, q: f32, sr: f32) {
        let g = (PI * freq.clamp(20., sr * 0.42) / sr).tan();
        self.k = 1. / q.clamp(0.5, 15.);
        self.a1 = 1. / (1. + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    fn sample(&mut self, x: f32, bp: bool) -> f32 {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2. * v1 - self.ic1;
        self.ic2 = 2. * v2 - self.ic2;
        if bp {
            v1 * self.k.sqrt()
        } else {
            v2
        }
    }
}
#[derive(Clone, Copy)]
struct Compressor {
    env: f32,
    gain: f32,
    attack: f32,
    release: f32,
}
impl Compressor {
    fn new(sr: f32) -> Self {
        Self {
            env: 0.,
            gain: 1.,
            attack: (-1. / (0.003 * sr)).exp(),
            release: (-1. / (0.09 * sr)).exp(),
        }
    }
    fn gain(&mut self, peak: f32, threshold: f32, ratio: f32) -> f32 {
        let k = if peak > self.env {
            self.attack
        } else {
            self.release
        };
        self.env = peak + k * (self.env - peak);
        let over = 20. * self.env.max(1e-9).log10() - threshold;
        let target = if over > 0. {
            10_f32.powf(-over * (1. - 1. / ratio) / 20.)
        } else {
            1.
        };
        let k = if target < self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain = target + k * (self.gain - target);
        self.gain
    }
}
pub struct Rack {
    sr: f32,
    filters: [Filter; 4],
    env: f32,
    target: f32,
    attack: f32,
    attack_left: usize,
    decay: f32,
    refresh: usize,
    comp: [Compressor; 5],
    prev: [f32; 4],
    smooth: [f32; 4],
    delay: Vec<f32>,
    head: usize,
    delay_target: f32,
    delay_length: f32,
    started: bool,
    pub metrics: [f32; 11],
}
impl Rack {
    pub fn new(sr: f32) -> Self {
        Self {
            sr,
            filters: [Filter::default(); 4],
            env: 0.,
            target: 0.,
            attack: 1.,
            attack_left: 0,
            decay: 0.,
            refresh: 0,
            comp: [Compressor::new(sr); 5],
            prev: [0.; 4],
            smooth: [0.; 4],
            delay: vec![0.; (sr * 33.) as usize + 1],
            head: 0,
            delay_target: 1.,
            delay_length: 1.,
            started: false,
            metrics: [0.; 11],
        }
    }
    pub fn configure(&mut self, s: &Song) {
        self.decay = (-1. / (s.fx.filter_decay * self.sr)).exp();
        self.delay_target = (self.sr * 60. / s.bpm / (if s.fx.delay_triplet { 3. } else { 4. })
            * s.fx.delay_steps as f32)
            .clamp(1., self.delay.len() as f32 - 2.);
        if !self.started {
            self.delay_length = self.delay_target;
        }
        self.refresh = 0;
    }
    pub fn clear(&mut self, echo: bool) {
        self.filters = [Filter::default(); 4];
        self.env = 0.;
        self.target = 0.;
        self.attack_left = 0;
        self.comp = [Compressor::new(self.sr); 5];
        self.prev = [0.; 4];
        self.smooth = [0.; 4];
        self.metrics = [0.; 11];
        if echo {
            self.delay.fill(0.);
            self.started = false;
        }
    }
    pub fn tick(&mut self, tick: usize, s: &Song) {
        let (division, levels) = super::patterns::PATTERNS[s.fx.filter_pattern];
        if tick % division == 0 {
            let level = levels[(tick / division) % levels.len()];
            if level > 0. {
                self.target = level;
                self.attack_left = (self.sr
                    * if s.fx.filter_pattern >= 12 {
                        0.024
                    } else {
                        0.004
                    }) as usize;
                self.attack = 1.
                    - (-1.
                        / (self.sr
                            * if s.fx.filter_pattern >= 12 {
                                0.006
                            } else {
                                0.001
                            }))
                    .exp();
            }
        }
    }
    pub fn sample(&mut self, ch: [f32; 4], s: &Song, click: f32) -> [f32; 2] {
        if self.attack_left > 0 {
            self.env += (self.target - self.env) * self.attack;
            self.attack_left -= 1;
        } else {
            self.target = 0.;
            self.env *= self.decay;
        }
        let f = &s.fx;
        if self.refresh == 0 {
            let cutoff =
                (f.filter_cutoff * 2_f32.powf(self.env * f.filter_amount * 5.)).min(20000.);
            for filter in &mut self.filters {
                filter.configure(cutoff, f.filter_q, self.sr);
            }
            self.refresh = 32;
        }
        self.refresh -= 1;
        let mut stereo = [0.; 2];
        let mut send = 0.;
        for i in 0..4 {
            let c = &s.channels[i];
            let mut x = if c.mute { 0. } else { ch[i] * c.level };
            self.metrics[i + 2] = self.metrics[i + 2].max(x.abs());
            if c.distortion && f.distortion_enabled && f.distortion > 0. {
                self.metrics[9] = self.metrics[9].max(x.abs());
                let drive = 1. + f.distortion * 24.;
                let before = self.prev[i];
                self.prev[i] = x;
                let mut sum = 0.;
                for sub in 1..=4 {
                    let z = (before + (x - before) * sub as f32 * 0.25) * drive;
                    sum += z.tanh() * (1. - f.distortion_shape)
                        + z.clamp(-1., 1.) * f.distortion_shape;
                }
                self.smooth[i] += 0.8 * (sum * 0.25 / drive.sqrt() - self.smooth[i]);
                x = self.smooth[i];
            }
            // ReBirth's inserts are serial: distortion, PCF, compressor.
            if c.filter && f.filter_enabled {
                self.metrics[7] = self.metrics[7].max(x.abs());
                x = self.filters[i].sample(x, f.filter_mode == "BP");
            }
            if c.compressor && f.compressor_enabled && !f.compressor_master {
                self.metrics[10] = self.metrics[10].max(x.abs());
                x *= self.comp[i].gain(x.abs(), f.compressor_threshold, f.compressor_ratio);
                self.metrics[6] = self.metrics[6].max(-20. * self.comp[i].gain.max(1e-6).log10());
            }
            let angle = (c.pan + 1.) * PI * 0.25;
            stereo[0] += x * angle.cos();
            stereo[1] += x * angle.sin();
            send += x * c.delay;
        }
        self.delay_length += (self.delay_target - self.delay_length) * 0.001;
        self.metrics[8] = self.metrics[8].max(send.abs());
        let read = (self.head as f32 + self.delay.len() as f32 - self.delay_length)
            % self.delay.len() as f32;
        let index = read as usize;
        let part = read - index as f32;
        let echo =
            self.delay[index] * (1. - part) + self.delay[(index + 1) % self.delay.len()] * part;
        self.delay[self.head] = (send + echo * f.delay_feedback).clamp(-8., 8.);
        self.head = (self.head + 1) % self.delay.len();
        self.started = true;
        if f.delay_enabled {
            let angle = (f.delay_pan + 1.) * PI * 0.25;
            stereo[0] += echo * f.delay_wet * angle.cos();
            stereo[1] += echo * f.delay_wet * angle.sin();
        }
        if f.compressor_enabled && f.compressor_master {
            self.metrics[10] = self.metrics[10].max(stereo[0].abs().max(stereo[1].abs()));
            let gain = self.comp[4].gain(
                stereo[0].abs().max(stereo[1].abs()),
                f.compressor_threshold,
                f.compressor_ratio,
            );
            for x in &mut stereo {
                *x *= gain;
            }
            self.metrics[6] = -20. * gain.max(1e-6).log10();
        }
        for j in 0..2 {
            let x = (stereo[j] + click) * f.master;
            assert!(x.is_finite(), "Non-finite audio state");
            stereo[j] = x.clamp(-1., 1.);
            self.metrics[j] = self.metrics[j].max(stereo[j].abs());
        }
        stereo
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delay_timing_and_pan() {
        let mut s = Song::default();
        s.bpm = 120.;
        s.fx.delay_steps = 3;
        s.fx.delay_triplet = true;
        s.fx.compressor_enabled = false;
        s.fx.delay_wet = 1.;
        s.fx.delay_pan = 1.;
        s.channels[0].delay = 1.;
        let mut r = Rack::new(48000.);
        r.configure(&s);
        assert_eq!(r.delay_target, 24000.);
        r.sample([1., 0., 0., 0.], &s, 0.);
        for _ in 1..24000 {
            assert_eq!(r.sample([0.; 4], &s, 0.), [0.; 2]);
        }
        let echo = r.sample([0.; 4], &s, 0.);
        assert!(echo[1] > 0.5);
        assert!(echo[0].abs() < 1e-6);
    }
    #[test]
    fn pcf_amount_zero_retains_filter() {
        let mut s = Song::default();
        s.fx.filter_amount = 0.;
        s.fx.filter_cutoff = 500.;
        s.channels[0].filter = true;
        s.channels[0].delay = 0.;
        s.fx.compressor_enabled = false;
        let mut r = Rack::new(48000.);
        r.configure(&s);
        let mut energy = 0.;
        for i in 0..4800 {
            let x = (2. * PI * 6000. * i as f32 / 48000.).sin();
            let y = r.sample([x, 0., 0., 0.], &s, 0.);
            energy += y[0] * y[0];
        }
        assert!(energy / 4800. < 0.001);
    }
    #[test]
    fn compression_has_envelope() {
        let mut c = Compressor::new(48000.);
        let first = c.gain(1., -24., 8.);
        for _ in 0..10000 {
            c.gain(1., -24., 8.);
        }
        assert!(first > 0.9);
        assert!(c.gain < 0.2);
    }
    #[test]
    fn lower_pattern_level_retriggers_downwards() {
        let mut s = Song::default();
        s.fx.filter_pattern = 48;
        let mut r = Rack::new(48000.);
        r.configure(&s);
        r.tick(0, &s);
        for _ in 0..1152 {
            r.sample([0.; 4], &s, 0.);
        }
        let high = r.env;
        assert!(high > 0.85);
        r.tick(1, &s);
        assert!((r.target - 0.182).abs() < 1e-6);
        for _ in 0..1152 {
            r.sample([0.; 4], &s, 0.);
        }
        assert!(r.env < high * 0.3 && r.env > 0.18);
    }
}
