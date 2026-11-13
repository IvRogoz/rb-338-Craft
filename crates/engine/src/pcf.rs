// PCF algorithm and pattern data derived from OpenRB-338, GPL-3.0, commit
// cbca4492c0224dffaa477346bc8860559f3d853f, crates/orb-engine/src/{pcf,tables}.
use rb_model::Song;
pub const PCF_LENGTHS: [u16; 55] = [
    16, 8, 4, 3, 2, 2, 4, 8, 16, 16, 16, 7, 16, 32, 19, 29, 4, 3, 32, 30, 30, 29, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 27, 10, 32, 32, 32, 32, 32, 32, 28, 32, 32, 32, 32, 32, 32, 32, 3, 5,
    7, 9, 11, 32, 4,
];

/// PCF pattern resolutions: 8, 16 or 32 steps per bar.
///
/// At 0x10033380.
pub const PCF_RESOLUTIONS: [u16; 55] = [
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    16, 16, 16, 16, 32, 8, 16,
];

/// PCF pattern steps: 0 = no trigger, otherwise the velocity (odd = slow attack).
///
/// At 0x100333F0.
pub const PCF_STEPS: [[u8; 32]; 55] = [
    [
        120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        120, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    [
        120, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    [
        120, 0, 120, 0, 120, 120, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    [
        120, 0, 120, 0, 0, 120, 0, 0, 120, 0, 0, 120, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    [
        0, 120, 0, 0, 120, 0, 0, 120, 0, 120, 0, 120, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    [
        120, 120, 0, 0, 120, 0, 120, 120, 0, 120, 0, 120, 120, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        120, 0, 120, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        1, 11, 21, 31, 42, 51, 61, 71, 81, 71, 61, 51, 41, 31, 21, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ],
    [
        1, 7, 13, 19, 25, 31, 37, 43, 49, 55, 61, 67, 73, 79, 85, 91, 95, 91, 85, 79, 73, 67, 61,
        55, 49, 43, 37, 31, 25, 19, 13, 7,
    ],
    [
        1, 11, 21, 31, 42, 51, 61, 71, 81, 91, 81, 71, 61, 51, 41, 31, 21, 11, 7, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ],
    [
        1, 7, 13, 19, 25, 31, 37, 43, 49, 55, 61, 67, 73, 79, 85, 91, 85, 79, 73, 67, 61, 55, 49,
        43, 37, 31, 25, 19, 9, 0, 0, 0,
    ],
    [
        91, 71, 51, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    [
        41, 91, 61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    [
        112, 0, 0, 0, 56, 0, 0, 0, 88, 0, 0, 0, 76, 0, 0, 0, 36, 0, 0, 0, 98, 0, 0, 0, 48, 0, 0, 0,
        84, 0, 0, 0,
    ],
    [
        112, 0, 0, 56, 0, 0, 88, 0, 0, 76, 0, 0, 36, 0, 0, 98, 0, 0, 48, 0, 0, 84, 0, 0, 58, 0, 0,
        96, 0, 0, 0, 0,
    ],
    [
        112, 0, 56, 0, 88, 0, 76, 0, 36, 0, 98, 0, 48, 0, 84, 0, 114, 0, 52, 0, 78, 0, 66, 0, 98,
        0, 105, 0, 68, 0, 0, 0,
    ],
    [
        112, 56, 88, 76, 36, 98, 48, 84, 114, 52, 78, 66, 98, 105, 68, 100, 58, 74, 88, 68, 92, 56,
        78, 62, 68, 52, 109, 68, 96, 0, 0, 0,
    ],
    [
        99, 0, 86, 0, 0, 98, 0, 0, 72, 0, 0, 87, 0, 0, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        34, 0, 49, 0, 75, 0, 5, 0, 0, 3, 40, 34, 0, 0, 0, 47, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        88, 0, 75, 0, 64, 0, 0, 74, 6, 0, 0, 75, 88, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        44, 22, 84, 83, 66, 23, 93, 25, 96, 36, 77, 23, 11, 36, 56, 75, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ],
    [
        72, 98, 0, 52, 54, 0, 16, 16, 0, 0, 65, 72, 95, 39, 0, 26, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    [
        0, 0, 46, 0, 0, 82, 14, 0, 85, 87, 75, 0, 53, 99, 44, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    [
        97, 0, 0, 97, 85, 0, 56, 0, 0, 74, 78, 0, 84, 96, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        0, 0, 49, 84, 0, 0, 0, 0, 75, 3, 40, 34, 0, 0, 78, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        35, 0, 0, 53, 65, 0, 0, 93, 0, 43, 0, 18, 58, 0, 65, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        99, 0, 86, 0, 0, 64, 0, 0, 74, 6, 0, 0, 75, 88, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        34, 0, 49, 0, 75, 0, 5, 0, 0, 3, 40, 34, 0, 0, 0, 47, 34, 93, 49, 84, 75, 32, 5, 0, 75, 3,
        40, 34, 65, 64, 78, 47,
    ],
    [
        88, 0, 75, 0, 64, 0, 0, 74, 6, 0, 0, 75, 88, 6, 0, 0, 26, 91, 26, 92, 63, 49, 65, 65, 16,
        52, 38, 58, 37, 52, 22, 63,
    ],
    [
        120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
        120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
    ],
    [
        120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 120, 120, 120, 120,
        120, 120, 2, 120, 2, 120, 120, 120, 2, 120, 2,
    ],
    [
        120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 120,
        120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
    ],
    [
        120, 120, 120, 120, 120, 120, 120, 120, 120, 2, 120, 2, 120, 2, 120, 120, 120, 2, 120, 2,
        120, 120, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
    ],
    [
        120, 2, 120, 2, 120, 2, 120, 120, 120, 120, 120, 120, 120, 120, 120, 2, 120, 2, 120, 2,
        120, 120, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
    ],
    [
        120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 120, 120, 120, 120,
        120, 120, 2, 120, 2, 120, 120, 120, 2, 120, 2,
    ],
    [
        120, 120, 120, 120, 120, 120, 120, 120, 120, 2, 120, 2, 120, 2, 120, 120, 120, 2, 120, 2,
        120, 120, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
    ],
    [
        120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120,
        120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120,
    ],
    [
        2, 4, 8, 12, 16, 20, 24, 28, 30, 34, 38, 42, 46, 50, 54, 58, 62, 66, 70, 74, 78, 82, 86,
        90, 92, 96, 100, 104, 108, 112, 116, 120,
    ],
    [
        120, 120, 120, 120, 0, 4, 8, 14, 18, 22, 26, 32, 36, 40, 44, 48, 54, 58, 62, 66, 72, 76,
        80, 84, 88, 94, 98, 102, 106, 112, 116, 120,
    ],
    [
        120, 116, 112, 108, 104, 100, 96, 92, 90, 86, 82, 78, 74, 70, 66, 62, 58, 54, 50, 46, 42,
        38, 34, 30, 28, 24, 20, 16, 12, 8, 4, 0,
    ],
    [
        120, 2, 120, 2, 120, 2, 120, 2, 121, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2, 120, 2,
        120, 2, 121, 2, 120, 2, 120, 2, 120, 2,
    ],
    [
        20, 20, 20, 20, 120, 20, 20, 20, 121, 20, 20, 20, 20, 20, 20, 120, 20, 20, 120, 20, 20, 20,
        20, 60, 60, 20, 60, 60, 60, 20, 120, 20,
    ],
    [
        119, 119, 20, 20, 20, 40, 120, 120, 40, 20, 20, 20, 40, 120, 120, 40, 120, 120, 20, 20,
        119, 119, 120, 120, 20, 20, 20, 20, 120, 120, 40, 50,
    ],
    [
        120, 20, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    [
        120, 20, 20, 80, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        20, 20, 80, 120, 80, 20, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        120, 80, 20, 20, 120, 120, 60, 4, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    [
        4, 120, 20, 120, 80, 90, 80, 70, 60, 50, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    [
        35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100, 105, 110, 105, 110, 95, 90, 85,
        80, 75, 70, 65, 60, 55, 50, 45, 40, 35, 30,
    ],
    [
        55, 80, 80, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
];
const DECAY: [f32; 16] = [
    0.99, 0.993, 0.995, 0.996, 0.997, 0.998, 0.999, 0.9993, 0.9995, 0.9996, 0.9997, 0.9998, 0.9999,
    0.99993, 0.99995, 1.0,
];

pub struct Pcf {
    sr: f64,
    clock: f64,
    group_clock: f64,
    tick: usize,
    step: usize,
    base: f32,
    smooth: [f32; 2],
    trigger: f64,
    env: f64,
    keep: f64,
    decay: f64,
    state: [f64; 2],
    mode: Option<bool>,
    coeff: [f32; 3],
    tables: Vec<[[f32; 3]; 512]>,
}
impl Pcf {
    pub fn new(sr: f32) -> Self {
        let mut tables = vec![[[0.; 3]; 512]; 16];
        let k = f64::from_bits(0x3ff03b93e0000000);
        let qbase = f64::from_bits(0x3ffc7ae140000000);
        let pi = f64::from_bits(0x400921fb54524550);
        for mode in 0..2 {
            for r in 0..8 {
                for i in 0..512 {
                    let rf = r as f64;
                    let fi = i as f64;
                    let w = (k.powf(fi) * 0.4 / k.powf(511.))
                        * (rf * (f32::from_bits(0x3e124925) as f64) * 0.4 + 0.6)
                        * (44100. / sr as f64);
                    let w = w.min(0.499);
                    let q = (fi * fi / 128000. - 0.3).clamp(0., 1.);
                    let d = q * (qbase.powf(rf) * 0.52) + (1. - q) * (rf / 6. + 0.5);
                    let c0 = (-w * pi / d).exp() * (w * pi * 2.).cos() * 2.;
                    let c1 = (-w * pi * 2. / d).exp();
                    let num = if mode == 0 {
                        (c1 + 1. - c0) * 2.
                    } else {
                        (w * pi).cos() * (1. - c0 + c1).sqrt() * 3.
                    };
                    let c2 = num / ((if d > 4. { 3. } else { d } + 1.) * 0.8);
                    tables[mode * 8 + r][i] = [c0 as f32, c1 as f32, c2 as f32];
                }
            }
        }
        Self {
            sr: sr as f64,
            clock: 0.,
            group_clock: 0.,
            tick: 0,
            step: usize::MAX,
            base: 0.,
            smooth: [0.; 2],
            trigger: 0.,
            env: 0.,
            keep: 0.,
            decay: 1.,
            state: [0.; 2],
            mode: None,
            coeff: [0.; 3],
            tables,
        }
    }
    pub fn clear(&mut self) {
        self.clock = 0.;
        self.group_clock = 0.;
        self.tick = 0;
        self.step = usize::MAX;
        self.base = 0.;
        self.smooth = [0.; 2];
        self.trigger = 0.;
        self.env = 0.;
        self.keep = 0.;
        self.decay = 1.;
        self.state = [0.; 2];
        self.mode = None;
    }
    pub fn tick(&mut self, tick: usize) {
        self.tick = tick;
    }
    pub fn advance(&mut self, s: &Song) {
        while self.clock <= 0. {
            self.clock += 64.;
            let freq =
                (((s.fx.filter_cutoff - 100.) / 7900. * 127.).round() as i32).clamp(0, 127) as f64;
            let w = f64::from_bits(0x3fb015bf8c180485);
            let raw_a = (-w / 2.).exp() * w.cos() * 2.;
            let raw_b = (-w).exp();
            let a = raw_a as f32;
            let b = raw_b as f32;
            let g = (1. - raw_a + raw_b) as f32;
            self.base = (a as f64 * self.smooth[0] as f64 + freq * 4. * g as f64
                - b as f64 * self.smooth[1] as f64) as f32;
            self.smooth = [self.base, self.smooth[0]];
            let p = s.fx.filter_pattern.min(54);
            let div = 32 / PCF_RESOLUTIONS[p] as usize;
            let step = (self.tick / div) % PCF_LENGTHS[p] as usize;
            if step != self.step {
                self.step = step;
                let d = (((s.fx.filter_decay - 0.1) / 9.9 * 127.).round() as usize).min(127);
                self.decay = DECAY[d / 8] as f64;
                let v = PCF_STEPS[p][step];
                if v != 0 {
                    let amt = (s.fx.filter_amount * 127.).round().clamp(0., 127.) as u32;
                    self.trigger = (((v as u32 * amt) >> 5) as f64).min(511. - self.base as f64);
                    self.keep = if v & 1 != 0 { 0.995_f32 as f64 } else { 0. };
                }
            }
            // Upstream stores intermediate state as f32 at each block boundary.
            self.trigger = self.trigger as f32 as f64;
            self.env = self.env as f32 as f64;
            self.state[0] = self.state[0] as f32 as f64;
            self.state[1] = self.state[1] as f32 as f64;
        }
        self.clock -= 44100. / self.sr;
        while self.group_clock <= 0. {
            self.group_clock += 4.;
            self.trigger *= self.decay;
            self.env = self.env * self.keep + (self.trigger + self.base as f64) * (1. - self.keep);
            let pos = self.env.clamp(0., 511.) as usize;
            let res = (((s.fx.filter_q - 0.5) / 14.5 * 127.).round() as usize).min(127);
            let mode = usize::from(s.fx.filter_mode == "BP");
            self.coeff = self.tables[mode * 8 + (res * 8 / 128)][pos];
        }
        self.group_clock -= 44100. / self.sr;
    }
    pub fn sample(&mut self, _channel: usize, x: f32, bp: bool) -> f32 {
        if self.mode != Some(bp) {
            self.state = [0.; 2];
            self.mode = Some(bp);
        }
        let [c0, c1, c2] = self.coeff.map(|v| v as f64);
        let old = self.state[0];
        let y = x as f64 * c2 + c0 * old - c1 * self.state[1];
        self.state = [y, old];
        (if bp { y - old } else { y }) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn song_pattern_audio_matches_upstream_checkpoints() {
        // Captured from the pinned OpenRB PCF using identical input and controls.
        // The final checkpoint follows pattern 13's two-bar wrap at 138 BPM.
        let expected = [
            (0, 0.0000011294921_f32),
            (10000, -0.0015622547),
            (20000, 0.025584184),
            (30000, -0.06752044),
            (40000, 0.04041784),
            (50000, -0.00038920977),
            (60000, -0.048203018),
            (70000, -0.018803552),
            (80000, -0.12746073),
            (90000, -0.02291619),
            (100000, 0.030760381),
            (110000, -0.04564086),
            (120000, -0.055376913),
            (130000, -0.06018785),
            (140000, 0.001602775),
            (150000, 0.03755612),
            (176000, -0.04688825),
        ];
        let mut song = Song::default();
        for (controller, value) in [1, 63, 93, 44, 13, 127, 1].into_iter().enumerate() {
            rb_model::rbs::apply_automation(
                &mut song,
                &rb_model::Automation {
                    tick: 0,
                    track: 7,
                    controller: controller as u8,
                    value,
                },
            );
        }
        let mut pcf = Pcf::new(44100.);
        let mut seed = 0x12345678_u32;
        let mut next = 0;
        for frame in 0..=176000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let mut input = ((seed >> 8) as f32 / 16777216. - 0.5) * 0.5;
            if frame == 0 {
                input += 0.5;
            }
            pcf.tick(frame * 138 * 8 / (60 * 44100));
            pcf.advance(&song);
            let output = pcf.sample(0, input, false);
            if next < expected.len() && frame == expected[next].0 {
                assert!(
                    (output - expected[next].1).abs() < 0.000002,
                    "frame {frame}: {output} vs {}",
                    expected[next].1
                );
                next += 1;
            }
        }
        assert_eq!(next, expected.len());
    }
    #[test]
    fn song_pattern_first_block_matches_upstream_control() {
        let mut song = Song::default();
        song.fx.filter_cutoff = 100. + 63. / 127. * 7900.;
        song.fx.filter_q = 0.5 + 93. / 127. * 14.5;
        song.fx.filter_amount = 44. / 127.;
        song.fx.filter_decay = 10.;
        song.fx.filter_pattern = 13;
        song.fx.filter_mode = "LP".into();
        let mut p = Pcf::new(44100.);
        p.tick(0);
        for _ in 0..64 {
            p.advance(&song);
            p.sample(0, 0., false);
        }
        assert!((p.base - 1.2048122).abs() < 0.00001);
        assert!((p.trigger - 1.).abs() < 0.00001);
        assert!((p.env - 0.16992223).abs() < 0.00001);
        assert_eq!(PCF_STEPS[13][16], 95);
    }
    #[test]
    fn sample_rates_are_finite_and_keep_time() {
        let mut song = Song::default();
        song.fx.filter_pattern = 13;
        song.fx.filter_cutoff = 100. + 63. / 127. * 7900.;
        song.fx.filter_amount = 44. / 127.;
        song.fx.filter_decay = 10.;
        song.fx.filter_q = 0.5 + 93. / 127. * 14.5;
        song.fx.filter_mode = "LP".into();
        for sr in [8000., 44100., 48000., 192000.] {
            let mut p = Pcf::new(sr);
            for i in 0..(sr as usize / 4) {
                p.tick((i as f64 * 138. * 8. / (60. * sr as f64)) as usize);
                p.advance(&song);
                let x = if i == 0 { 1. } else { 0. };
                assert!(p.sample(0, x, false).is_finite());
            }
            assert!((p.base - 252.).abs() < 1., "{sr}: {}", p.base);
            assert_eq!(p.step, 2);
            p.clear();
            p.tick(0);
            p.advance(&song);
            assert!((p.base - 1.2048122).abs() < 0.00001);
            assert!(p.sample(0, 1., false).is_finite());
        }
    }

    #[test]
    fn switching_filter_mode_clears_previous_mode_state() {
        let mut p = Pcf::new(44100.);
        let mut song = Song::default();
        song.fx.filter_mode = "LP".into();
        for _ in 0..32 {
            p.advance(&song);
            p.sample(0, 1., false);
        }
        assert_ne!(p.state, [0.; 2]);
        song.fx.filter_mode = "BP".into();
        p.advance(&song);
        assert_eq!(p.sample(0, 0., true), 0.);
    }
    #[test]
    fn routing_change_keeps_filter_history() {
        let mut song = Song::default();
        song.fx.filter_mode = "LP".into();
        let mut switched = Pcf::new(44100.);
        let mut unchanged = Pcf::new(44100.);
        for i in 0..128 {
            switched.advance(&song);
            unchanged.advance(&song);
            let input = if i == 0 { 1. } else { 0. };
            let actual = switched.sample(if i < 64 { 0 } else { 3 }, input, false);
            let expected = unchanged.sample(0, input, false);
            assert_eq!(actual, expected);
        }
    }
}
