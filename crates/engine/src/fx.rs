// Effects and table data derived from OpenRB-338, GPL-3.0, commit
// cbca4492c0224dffaa477346bc8860559f3d853f.
use rb_model::Song;

#[path = "pcf.rs"]
mod pcf;
#[derive(Clone, Copy)]
struct Compressor {
    counter: u16, x: f32, off: f32, env: f32, gain: f32, floor: f32, hold: u16,
}
impl Compressor {
    fn new() -> Self { Self { counter: 0, x: 1., off: 1., env: 1., gain: 0., floor: 1000., hold: 0 } }
    fn process(&mut self, buses: &mut [&mut [f32; 64]], amount: f32, threshold: f32) {
        if self.counter & 15 == 0 {
            let a = byte(((amount - 1.) / 19.).clamp(0., 1.)) as f64 * f32::from_bits(0x3c010204) as f64;
            let t = byte(10_f32.powf(threshold / 20.)) as f64;
            self.off = (1. - a * 0.99) as f32;
            self.x = (a * (t * 0.03125 + 1.) * 0.3662109375) as f32;
            self.floor = ((t * f32::from_bits(0x3c010204) as f64) * 80000. / 32768. + 0.01220703125) as f32;
        }
        self.counter = self.counter.wrapping_add(1);
        let mut env = self.env as f64;
        let mut gain = self.gain as f64;
        for i in (0..64).step_by(4) {
            let mut detector = buses[0][i+1] as f64 + buses[0][i+3] as f64;
            for b in &buses[1..] { detector = detector + b[i+1] as f64 + b[i+3] as f64; }
            detector = detector.abs();
            if detector > env { self.hold = 300; env += (detector - env) * f32::from_bits(0x3c23d70a) as f64; }
            else if self.hold == 0 { env = env * f32::from_bits(0x3f7f9724) as f64 + f32::from_bits(0x358637bd) as f64; }
            else { self.hold -= 1; }
            env = env.max(self.floor as f64);
            let step = ((self.x as f64 / env + self.off as f64) - gain) * 0.25;
            for k in 0..4 { gain += step; for b in buses.iter_mut() { b[i+k] = (b[i+k] as f64 * gain) as f32; } }
        }
        self.env = env as f32; self.gain = gain as f32;
    }
}
// The level and pan curves use the original byte-indexed tables.
fn byte(value: f32) -> usize { (value.clamp(0., 1.) * 127.).round() as usize }
fn level_at(i: usize) -> f32 {
    if i == 0 { return 0.; }
    let mut v = 1_f32;
    for _ in i..127 { v = (v as f64 - (1_f32 / 24.) as f64 * v as f64) as f32; }
    v
}
fn level_gain(value: f32) -> f32 { level_at(byte(value)) }
fn mixer_gain(value: f32) -> f32 { (2. * level_gain(value) as f64 + 0.000001_f32 as f64) as f32 }
fn pan_gains(pan: f32) -> [f32; 2] {
    let p = ((pan.clamp(-1., 1.) * 63.5 + 63.5).round() as i32).clamp(0, 127);
    let d = p - 64;
    let cut = (127 - ((d * d) >> 5)).clamp(0, 127) as usize;
    [if d > 0 { level_at(cut) } else { 1. }, if d < 0 { level_at(cut) } else { 1. }]
}
const GRID: [[f32; 5]; 5] = [
    [1.2, 0.4, 0.28, 0.2, 0.15], [0.35, 0.2, 0.15, 0.14, 0.12],
    [0.2, 0.15, 0.13, 0.12, 0.12], [0.15, 0.12, 0.11, 0.1, 0.1],
    [0.12, 0.1, 0.08, 0.08, 0.08],
];
const SHAPER: &[u8; 32768] = include_bytes!("fx_tables.bin");
fn shape(row: usize, v: f64) -> f64 {
    let value = |i: usize| {
        let at = (row * 256 + i) * 4;
        f32::from_le_bytes(SHAPER[at..at + 4].try_into().unwrap()) as f64
    };
    if v < 0. { value(0) } else if v > 254. { value(254) } else {
        let i = v as usize; (v - i as f64) * (value(i + 1) - value(i)) + value(i)
    }
}
fn dist_grid(shape: usize, amount: usize) -> f64 {
    let c = |x: usize| x as f64 * f32::from_bits(0x3c010204) as f64 * 3.999;
    let (s, a) = (c(shape), c(amount));
    let (i, j) = (s as usize, a as usize);
    let (fs, fa) = (s - i as f64, a - j as f64);
    let g = |r: usize, c: usize| GRID[r][c] as f64;
    let top = (g(i+1,j+1)-g(i+1,j))*fa+g(i+1,j);
    let bot = (g(i,j+1)-g(i,j))*fa+g(i,j);
    fs*(top-bot)+bot
}
pub struct Rack {
    mixer_counter: u16, configured: bool, block_pending: bool, routing: [[bool; 3]; 4], master_comp: bool, levels: [f32; 4], sends: [f32; 4], pans: [[f32; 2]; 4], master: f32,
    mix_gain: [[f32; 2]; 4], master_gain: f32, pcf: pcf::Pcf, comp: Compressor,
    dist_counter: u16, dist_in: f32, dist_out: f32, dist_target_in: f32, dist_target_out: f32, dist_shape: usize,
    delay: Vec<f32>, head: usize, delay_length: usize, delay_counter: u16,
    delay_gain: [f32; 2], delay_target: [f32; 2], delay_feedback: f32, started: bool,
    pub metrics: [f32; 11],
}
impl Rack {
    pub fn new(sr: f32) -> Self {
        Self { mixer_counter: 10, configured: false, block_pending: false, routing: [[false; 3]; 4], master_comp: false, levels: [0.;4], sends: [0.;4], pans: [[0.;2];4], master: 0., mix_gain: [[0.;2];4], master_gain: 0.,
            pcf: pcf::Pcf::new(sr), comp: Compressor::new(), dist_counter: 0, dist_in: 0., dist_out: 0.,
            dist_target_in: 0., dist_target_out: 0., dist_shape: 0,
            delay: vec![0.; 131072], head: 0, delay_length: 0, delay_counter: 0,
            delay_gain: [0.;2], delay_target: [0.;2], delay_feedback: 0., started: false, metrics: [0.;11] }
    }
    pub fn configure(&mut self, s: &Song) {
        if !self.configured { self.update_mixer(s); self.configured = true; }
    }
    fn update_mixer(&mut self, s: &Song) {
        self.master = mixer_gain(s.fx.master);
        self.master_comp = s.fx.compressor_master;
        for i in 0..4 {
            let c = &s.channels[i];
            self.routing[i] = [c.distortion, c.filter, c.compressor];
            self.levels[i] = level_gain(if c.mute { 0. } else { c.level });

            let p = ((c.pan.clamp(-1.,1.) * 63.5 + 63.5).round() as i32).clamp(0,127);
            let d = p - 64; let level = byte(if c.mute { 0. } else { c.level }) as i32;
            let cut = (level - ((d*d)>>5)).clamp(0,127) as usize;
            self.pans[i] = [if d > 0 { level_at(cut) } else { level_at(level as usize) },
                            if d < 0 { level_at(cut) } else { level_at(level as usize) }];
            let send_level = if s.vintage { (self.pans[i][0] as f64 + self.pans[i][1] as f64) * 0.5 } else { self.levels[i] as f64 };
            self.sends[i] = (send_level * level_gain(c.delay) as f64 + 0.000001_f32 as f64) as f32;
            self.pans[i] = self.pans[i].map(|x| (x as f64 + 0.000001_f32 as f64) as f32);
        }
    }
    pub fn clear(&mut self, echo: bool) {
        self.block_pending = false;
        self.pcf.clear(); self.comp = Compressor::new(); self.mix_gain = [[0.;2];4]; self.master_gain = 0.;
        self.metrics = [0.;11];
        if echo { self.delay.fill(f32::from_bits(0x358637bd)); self.started = false; }
    }
    pub fn tick(&mut self, tick: usize, _s: &Song) { self.pcf.tick(tick); }
    fn distort(&mut self, bus: &mut [f32;64], s: &Song) {
        let f = &s.fx;

                if self.dist_counter & 15 == 0 {
                    let amount = byte(f.distortion); self.dist_shape = byte(f.distortion_shape);
                    self.dist_target_in = ((level_at(amount) as f64 + 0.0625) * 2048.) as f32;
                    self.dist_target_out = (dist_grid(self.dist_shape, amount) * 5. / 256.) as f32;
                }
                self.dist_counter = self.dist_counter.wrapping_add(1);
                self.dist_in = (self.dist_in as f64 * 0.9 + self.dist_target_in as f64 * 0.1) as f32;
                self.dist_out = (self.dist_out as f64 * 0.9 + self.dist_target_out as f64 * 0.1) as f32;
                let amount = byte(f.distortion);
                for x in bus.iter_mut() {
                    self.metrics[9] = self.metrics[9].max(x.abs());
                    *x = if self.dist_shape == 0 {
                        let gain = 16. * level_at(amount) as f64 + 1.;
                        let ceiling = 1. / (16. * level_at(amount.min(90)) as f64 + 1.);
                        (gain * *x as f64).clamp(-ceiling, ceiling) as f32
                    } else { (shape(self.dist_shape >> 2, self.dist_in as f64 * *x as f64 + 128.) * self.dist_out as f64) as f32 };
                }
                }
    fn process_delay(&mut self, send: &[f32;64], output: &mut [[f32;2];64], s: &Song) {
        let f = &s.fx;
        if f.delay_enabled {
            if self.delay_counter & 15 == 0 {
                let tempo = (s.bpm * 1000.) as u32;
                let divisor = if f.delay_triplet { 3 * tempo / 100 } else { 4 * tempo / 100 };
                self.delay_length = (26_460_000u32.wrapping_mul(f.delay_steps as u32).checked_div(divisor).unwrap_or(0x1ffff).min(0x1ffff)) as usize;
                self.delay_feedback = (byte(f.delay_feedback) as f64 / 127.) as f32;
                self.delay_target = pan_gains(f.delay_pan).map(|x| (x as f64 + 0.000001) as f32);
            }
            self.delay_counter = self.delay_counter.wrapping_add(1);
            let mut gain = [0_f64;2];
            for j in 0..2 { gain[j] = self.delay_gain[j] as f64 * 0.9 + self.delay_target[j] as f64 * 0.1; self.delay_gain[j] = gain[j] as f32; }
            for n in 0..64 {
                let idx = self.head.wrapping_sub(self.delay_length) & 131071;
                let echo = self.delay[idx] as f64;
                self.delay[self.head] = (send[n] as f64 + echo * self.delay_feedback as f64) as f32;
                self.head = (self.head + 1) & 131071;
                for j in 0..2 { output[n][j] = (output[n][j] as f64 + echo * gain[j] * f.delay_wet as f64) as f32; }
                self.metrics[8] = self.metrics[8].max(send[n].abs());
            }
            self.started = true;
        } else if self.started { self.delay.fill(f32::from_bits(0x358637bd)); self.started = false; }
    }
    pub fn begin_block(&mut self, s: &Song) {
        if !self.block_pending {
            if self.mixer_counter & 15 == 0 { self.update_mixer(s); }
            self.mixer_counter = self.mixer_counter.wrapping_add(1);
            self.block_pending = true;
        }
    }
    pub fn process_block(&mut self, input: &[[f32;4];64], s: &Song, clicks: &[f32;64]) -> [[f32;2];64] {
        self.begin_block(s);
        self.block_pending = false;
        let f = &s.fx;
        let guard = f32::from_bits(0x358637bd);
        let mut output = [[guard;2];64]; let mut send = [guard;64];
        for i in 0..4 {
            let mut bus = [0_f32;64];
            for n in 0..64 { bus[n] = input[n][i]; }
            if self.routing[i][0] && f.distortion_enabled { self.distort(&mut bus, s); }
            if self.routing[i][1] && f.filter_enabled { for x in &mut bus { self.metrics[7] = self.metrics[7].max(x.abs()); self.pcf.advance(s); *x = self.pcf.sample(i, *x, f.filter_mode == "BP"); } }
            if self.routing[i][2] && f.compressor_enabled && !self.master_comp {
                for x in &bus { self.metrics[10] = self.metrics[10].max(x.abs()); }
                self.comp.process(&mut [&mut bus], f.compressor_ratio, f.compressor_threshold);
                self.metrics[6] = (-20. * self.comp.gain.max(1e-6).log10()).max(0.);
            }
            let mut start = [0_f64;2]; let mut step = [0_f64;2];
            for j in 0..2 {
                start[j] = self.mix_gain[i][j] as f64;
                let target = self.pans[i][j] as f64;
                let new = start[j] * 0.93 + target * 0.07;
                self.mix_gain[i][j] = new as f32; step[j] = (new - start[j]) / 32.;
            }
            let sg = self.sends[i] as f64;
            for n in 0..64 {
                let x = bus[n] as f64; send[n] = (send[n] as f64 + x * sg) as f32;
                self.metrics[i+2] = self.metrics[i+2].max((x * self.levels[i] as f64).abs() as f32);
                for j in 0..2 { output[n][j] = (output[n][j] as f64 + x * (start[j] + step[j] * (n / 2) as f64)) as f32; }
            }
        }
        self.process_delay(&send, &mut output, s);
        if f.compressor_enabled && self.master_comp {
            for x in &output { self.metrics[10] = self.metrics[10].max(x[0].abs().max(x[1].abs())); }
            let mut left = output.map(|x| x[0]); let mut right = output.map(|x| x[1]);
            self.comp.process(&mut [&mut left, &mut right], f.compressor_ratio, f.compressor_threshold);
            for n in 0..64 { output[n] = [left[n],right[n]]; }
            self.metrics[6] = (-20. * self.comp.gain.max(1e-6).log10()).max(0.);
        }
        let m = self.master_gain as f64 * 0.93 + self.master as f64 * 0.07;
        self.master_gain = m as f32;
        for n in 0..64 { for j in 0..2 {
            output[n][j] = (((output[n][j] + clicks[n]) as f64 * m) as f32).clamp(-1., 1.);
            self.metrics[j] = self.metrics[j].max(output[n][j].abs());
        } }
        output
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixer_routing_latches_before_block_events() {
        let mut song = Song::default();
        song.channels[0].distortion = false;
        song.fx.compressor_master = false;
        let mut rack = Rack::new(44100.);
        rack.configure(&song);
        rack.mixer_counter = 16;
        rack.begin_block(&song);
        song.channels[0].distortion = true;
        song.fx.compressor_master = true;
        rack.begin_block(&song);
        assert_eq!(rack.mixer_counter, 17);
        assert!(!rack.routing[0][0]);
        assert!(!rack.master_comp);
        rack.process_block(&[[0.; 4]; 64], &song, &[0.; 64]);
        for _ in 0..15 { rack.process_block(&[[0.; 4]; 64], &song, &[0.; 64]); }
        rack.begin_block(&song);
        assert!(rack.routing[0][0]);
        assert!(rack.master_comp);
    }
    #[test]
    fn delay_matches_reference_impulse() {
        let mut song = Song::default();
        song.bpm = 120.; song.fx.delay_enabled = true; song.fx.delay_steps = 1;
        song.fx.delay_triplet = false; song.fx.delay_feedback = 80. / 127.;
        song.fx.delay_pan = 0.; song.fx.delay_wet = 1.;
        let mut rack = Rack::new(44100.);
        for block in 0..=86 {
            let mut input = [0.; 64];
            if block == 0 { input[0] = 0.25; }
            let mut output = [[0.; 2]; 64];
            rack.process_delay(&input, &mut output, &song);
            if block == 86 { assert_eq!(output[8][0].to_bits(), 0x3e7ff937); }
        }
    }
    #[test]
    fn distortion_and_compressor_match_reference_vectors() {
        let mut song = Song::default();
        song.fx.distortion = 70. / 127.;
        song.fx.distortion_shape = 38. / 127.;
        let mut rack = Rack::new(44100.);
        let mut bus = [0.25; 64];
        rack.distort(&mut bus, &song);
        assert_eq!(bus[0].to_bits(), 0x3c58fd73);
        let mut comp = Compressor::new();
        let mut bus = [0.25; 64];
        comp.process(&mut [&mut bus], 1. + 19. * 70. / 127., 20. * (64_f32 / 127.).log10());
        assert_eq!(bus[0].to_bits(), 0x3d71121f);
        assert_eq!(bus[63].to_bits(), 0x3e71121f);
    }
}
