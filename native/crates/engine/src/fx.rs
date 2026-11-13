use rb_model::Song;

#[path = "pcf.rs"]
mod pcf;
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
        // The reference Amount control blends automatic level gain, not a ratio-only limiter.
        let amount = ((ratio - 1.) / 19.).clamp(0., 1.);
        let threshold = 10_f32.powf(threshold / 20.).clamp(0., 1.);
        let floor = 0.0122 + threshold * 2.44;
        let level = 0.366 + threshold * 1.454;
        let target = (1. - amount) + amount * level / self.env.max(floor);
        let k = if target < self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain = target + k * (self.gain - target);
        self.gain
    }
}
// Reference faders are logarithmic; retain the calibration point at knob 100.
fn level_gain(value: f32) -> f32 {
    if value <= 0. { 0. } else { ((value - 1.) * 5.406).exp() }
}
fn mixer_gain(value: f32) -> f32 {
    level_gain(value) * 2.485
}
fn pan_gains(pan: f32) -> [f32; 2] {
    let attenuation = if pan.abs() >= 0.999 { 0. } else { (-5.448 * pan * pan).exp() };
    [if pan > 0. { attenuation } else { 1. }, if pan < 0. { attenuation } else { 1. }]
}
pub struct Rack {
    sr: f32,
    levels: [f32; 4],
    sends: [f32; 4],
    delay_pan: [f32; 2],
    pans: [[f32; 2]; 4],
    master: f32,
    pcf: pcf::Pcf,
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
            levels: [0.; 4],
            sends: [0.; 4],
            delay_pan: [1.; 2],
            pans: [[0.; 2]; 4],
            master: 0.,
            pcf: pcf::Pcf::new(sr),
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
        self.master = mixer_gain(s.fx.master);
        self.delay_pan = pan_gains(s.fx.delay_pan);
        for i in 0..4 {
            self.levels[i] = mixer_gain(s.channels[i].level);
            self.sends[i] = level_gain(s.channels[i].delay);
            self.pans[i] = pan_gains(s.channels[i].pan);
        }
        self.delay_target = (self.sr * 60. / s.bpm / (if s.fx.delay_triplet { 3. } else { 4. })
            * s.fx.delay_steps as f32)
            .clamp(1., self.delay.len() as f32 - 2.);
        if !self.started {
            self.delay_length = self.delay_target;
        }
    }
    pub fn clear(&mut self, echo: bool) {
        self.pcf.clear();
        self.comp = [Compressor::new(self.sr); 5];
        self.prev = [0.; 4];
        self.smooth = [0.; 4];
        self.metrics = [0.; 11];
        if echo {
            self.delay.fill(0.);
            self.started = false;
        }
    }
    pub fn tick(&mut self, tick: usize, _s: &Song) {
        self.pcf.tick(tick);
    }
    pub fn sample(&mut self, ch: [f32; 4], s: &Song, click: f32) -> [f32; 2] {
        let f = &s.fx;
        if f.filter_enabled && s.channels.iter().any(|c| c.filter) {
            self.pcf.advance(s);
        }
        let mut stereo = [0.; 2];
        let mut send = 0.;
        for i in 0..4 {
            let c = &s.channels[i];
            // Inserts precede the fader; convert the normalized voices to the insert's headroom.
            let mut x = if c.mute { 0. } else { ch[i] * 2.18 };
            if c.distortion && f.distortion_enabled && f.distortion > 0. {
                self.metrics[9] = self.metrics[9].max(x.abs());
                let drive = 1. + level_gain(f.distortion) * 16.;
                let ceiling = 1. / (1. + level_gain(f.distortion.min(90. / 127.)) * 16.);
                let before = self.prev[i];
                self.prev[i] = x;
                let mut sum = 0.;
                for sub in 1..=4 {
                    let z = (before + (x - before) * sub as f32 * 0.25) * drive;
                    sum += z.clamp(-ceiling, ceiling) * (1. - f.distortion_shape)
                        + (z / ceiling).tanh() * ceiling * f.distortion_shape;
                }
                self.smooth[i] += 0.8 * (sum * 0.25 - self.smooth[i]);
                x = self.smooth[i];
            }
            // ReBirth's inserts are serial: distortion, PCF, compressor.
            if c.filter && f.filter_enabled {
                self.metrics[7] = self.metrics[7].max(x.abs());
                x = self.pcf.sample(i, x, f.filter_mode == "BP");
            }
            if c.compressor && f.compressor_enabled && !f.compressor_master {
                self.metrics[10] = self.metrics[10].max(x.abs());
                x *= self.comp[i].gain(x.abs() * 2., f.compressor_threshold, f.compressor_ratio);
                self.metrics[6] = self.metrics[6].max(-20. * self.comp[i].gain.max(1e-6).log10());
            }
            x *= self.levels[i] / 2.18;
            self.metrics[i + 2] = self.metrics[i + 2].max(x.abs());
            stereo[0] += x * self.pans[i][0] * std::f32::consts::FRAC_1_SQRT_2;
            stereo[1] += x * self.pans[i][1] * std::f32::consts::FRAC_1_SQRT_2;
            send += x * self.sends[i];
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
            stereo[0] += echo * f.delay_wet * self.delay_pan[0] * std::f32::consts::FRAC_1_SQRT_2;
            stereo[1] += echo * f.delay_wet * self.delay_pan[1] * std::f32::consts::FRAC_1_SQRT_2;
        }
        if f.compressor_enabled && f.compressor_master {
            self.metrics[10] = self.metrics[10].max(stereo[0].abs().max(stereo[1].abs()));
            let gain = self.comp[4].gain(
                (stereo[0].abs() + stereo[1].abs()) * 2.48,
                f.compressor_threshold,
                f.compressor_ratio,
            );
            for x in &mut stereo {
                *x *= gain;
            }
            self.metrics[6] = (-20. * gain.max(1e-6).log10()).max(0.);
        }
        for j in 0..2 {
            let x = (stereo[j] + click) * self.master;
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
        assert!(echo[1] > 0.35);
        assert!(echo[0].abs() < 1e-6);
    }
    #[test]
    fn logarithmic_faders_and_pan_keep_center_and_mute_endpoints() {
        assert_eq!(mixer_gain(0.), 0.);
        assert!((mixer_gain(100. / 127.) - 100. / 127.).abs() < 0.002);
        assert!(mixer_gain(0.5) < 0.25);
        assert_eq!(pan_gains(-1.), [1., 0.]);
        assert_eq!(pan_gains(1.), [0., 1.]);
        assert_eq!(pan_gains(0.), [1., 1.]);
    }
    #[test]
    fn pcf_bypass_pauses_state_and_clear_resets_it() {
        let mut song = Song::default();
        song.fx.filter_enabled = false;
        song.fx.delay_enabled = false;
        song.fx.compressor_enabled = false;
        song.channels[0].filter = true;
        let mut rack = Rack::new(44100.);
        rack.configure(&song);
        for _ in 0..64 { rack.sample([1., 0., 0., 0.], &song, 0.); }
        song.fx.filter_enabled = true;
        let first = rack.sample([1., 0., 0., 0.], &song, 0.);
        rack.clear(true);
        let reset = rack.sample([1., 0., 0., 0.], &song, 0.);
        assert_eq!(first, reset);
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
            let x = (2. * std::f32::consts::PI * 6000. * i as f32 / 48000.).sin();
            let y = r.sample([x, 0., 0., 0.], &s, 0.);
            energy += y[0] * y[0];
        }
        assert!(energy / 4800. < 0.001);
    }
    #[test]
    fn compression_has_envelope() {
        let mut c = Compressor::new(48000.);
        let first = c.gain(1., -40., 20.);
        for _ in 0..10000 {
            c.gain(1., -40., 20.);
        }
        assert!(first > 0.9);
        assert!(c.gain < 0.4);
        let mut quiet = Compressor::new(48000.);
        for _ in 0..10000 { quiet.gain(0.01, -40., 20.); }
        assert!(quiet.gain > 5. && quiet.gain.is_finite());
        for _ in 0..10000 { quiet.gain(0.01, -40., 1.); }
        assert!((quiet.gain - 1.).abs() < 1e-4);
    }
}
