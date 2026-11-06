//! Standard MIDI remote-control decoding, based on ReBirth manual pp. 195–202.
//! This module performs no device access and schedules no audio.
use rb_model::Automation;

#[derive(Clone, Debug, PartialEq)]
pub enum MidiEvent {
    Control(Automation),
    MasterLevel(f32),
    Shuffle(f32),
    InstrumentSelect { kit: usize, value: u8 },
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    Clock,
    Start,
    Continue,
    Stop,
    SongPosition { sixteenths: u16 },
}
/// `channel` is zero based. Real-time/system messages are channel-independent.
pub fn decode(bytes: &[u8], channel: u8) -> Option<MidiEvent> {
    let status = *bytes.first()?;
    match status {
        0xf8 => return Some(MidiEvent::Clock),
        0xfa => return Some(MidiEvent::Start),
        0xfb => return Some(MidiEvent::Continue),
        0xfc => return Some(MidiEvent::Stop),
        0xf2 if bytes.len() >= 3 && bytes[1] < 128 && bytes[2] < 128 => {
            return Some(MidiEvent::SongPosition {
                sixteenths: bytes[1] as u16 | ((bytes[2] as u16) << 7),
            })
        }
        _ => {}
    }
    if channel > 15
        || status & 15 != channel
        || bytes.len() < 3
        || bytes[1] >= 128
        || bytes[2] >= 128
    {
        return None;
    }
    match status & 0xf0 {
        0x80 => Some(MidiEvent::NoteOff { note: bytes[1] }),
        0x90 if bytes[2] == 0 => Some(MidiEvent::NoteOff { note: bytes[1] }),
        0x90 => Some(MidiEvent::NoteOn {
            note: bytes[1],
            velocity: bytes[2],
        }),
        0xb0 => control(bytes[1], bytes[2]),
        _ => None,
    }
}
fn control(cc: u8, value: u8) -> Option<MidiEvent> {
    let (track, controller, v) = match cc {
        7 => return Some(MidiEvent::MasterLevel(value as f32 / 127.)),
        11..=22 => {
            let n = cc - 11;
            (0, 6 + (n / 3) * 6 + n % 3, value)
        }
        23..=36 => {
            let n = cc - 23;
            (1 + n / 7, if n % 7 == 0 { 8 } else { 1 + n % 7 }, value)
        }
        37..=63 => (3, cc - 35, value),
        64 => return Some(MidiEvent::InstrumentSelect { kit: 0, value }),
        65..=82 => (4, cc - 63, value),
        83 => (4, 22, value),
        84 => (4, 20, value),
        85 => (4, 21, value),
        86 => (4, 23, value),
        87 => (4, 24, value),
        88..=92 => (4, cc - 63, value),
        93 => return Some(MidiEvent::InstrumentSelect { kit: 1, value }),
        94 => (7, 4, ((value as u16 * 55) / 128) as u8),
        95 => (7, 6, value),
        96..=98 => (7, cc - 95, value),
        99 => (7, 5, value),
        100 => (5, 1, 1 + ((value as u16 * 32) / 128) as u8),
        101 => (5, 2, value),
        102 => (5, 4, value),
        103 => (5, 3, value),
        104 => (6, 2, value),
        105 => (6, 1, value),
        106 => (8, 1, value),
        107 => (8, 2, value),
        108 => return Some(MidiEvent::Shuffle(value as f32 / 127. * 60.)),
        _ => return None,
    };
    Some(MidiEvent::Control(Automation {
        tick: 0,
        track,
        controller,
        value: v,
    }))
}
/// Notes 64–96 are panel actions; notes 12–63 depend on keyboard mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyAction {
    Focus(usize),
    Play,
    Stop,
    Record,
    BarBack,
    BarForward,
    ToggleEffect(u8),
    ToggleMix(usize),
    ToggleDist(usize),
    ToggleFilter(usize),
    ToggleCompressor(usize),
    ToggleMasterCompressor,
    SelectPatternMode,
    ProgramSynthMode,
    SelectOrProgram,
}
pub fn panel_key(note: u8) -> Option<KeyAction> {
    Some(match note {
        64 => KeyAction::SelectOrProgram,
        65..=68 => KeyAction::Focus((note - 65) as usize),
        69 => KeyAction::Play,
        70 => KeyAction::Record,
        71 => KeyAction::Stop,
        72 => KeyAction::BarBack,
        73 => KeyAction::BarForward,
        74..=77 => KeyAction::ToggleEffect(note - 74),
        78..=93 => {
            let n = note - 78;
            match n % 4 {
                0 => KeyAction::ToggleMix((n / 4) as usize),
                1 => KeyAction::ToggleDist((n / 4) as usize),
                2 => KeyAction::ToggleFilter((n / 4) as usize),
                _ => KeyAction::ToggleCompressor((n / 4) as usize),
            }
        }
        94 => KeyAction::ToggleMasterCompressor,
        95 => KeyAction::ProgramSynthMode,
        96 => KeyAction::SelectPatternMode,
        _ => return None,
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PatternKey {
    Toggle(usize),
    Bank(usize, usize),
    Pattern(usize, usize),
}
pub fn pattern_key(note: u8) -> Option<PatternKey> {
    if !(12..=63).contains(&note) {
        return None;
    }
    let n = (note - 12) as usize;
    let track = n / 13;
    let key = n % 13;
    Some(if key == 0 {
        PatternKey::Toggle(track)
    } else if key <= 4 {
        PatternKey::Bank(track, key - 1)
    } else {
        PatternKey::Pattern(track, key - 5)
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SynthKey {
    Pitch(u8),
    OctaveDown,
    OctaveUp,
    Accent,
    Slide,
    NotePause,
    Back,
    Step,
    PitchMode,
}
pub fn synth_key(note: u8) -> Option<SynthKey> {
    Some(match note {
        12..=24 => SynthKey::Pitch((note - 12) % 12),
        25 => SynthKey::OctaveDown,
        26 => SynthKey::OctaveUp,
        27 => SynthKey::Accent,
        28 => SynthKey::Slide,
        29 => SynthKey::NotePause,
        30 => SynthKey::Back,
        31 => SynthKey::Step,
        32 => SynthKey::PitchMode,
        _ => return None,
    })
}
/// Track external timing in MIDI clocks (24 per quarter). Timestamp is milliseconds.
#[derive(Default, Debug)]
pub struct MidiClock {
    pub running: bool,
    pub clocks: u64,
    last_ms: Option<f64>,
    interval_ms: Option<f64>,
}
impl MidiClock {
    pub fn process(&mut self, event: &MidiEvent, timestamp_ms: f64) {
        match event {
            MidiEvent::Start => {
                self.running = true;
                self.clocks = 0;
                self.last_ms = None;
                self.interval_ms = None
            }
            MidiEvent::Continue => {
                self.running = true;
                self.last_ms = None;
            }
            MidiEvent::Stop => {
                self.running = false;
                self.last_ms = None;
            }
            MidiEvent::SongPosition { sixteenths } => self.clocks = *sixteenths as u64 * 6,
            MidiEvent::Clock => {
                if timestamp_ms.is_finite() {
                    if let Some(last) = self.last_ms {
                        let dt = timestamp_ms - last;
                        if dt > 0. && dt < 1000. {
                            self.interval_ms = Some(match self.interval_ms {
                                Some(old) => old * 0.85 + dt * 0.15,
                                None => dt,
                            });
                        }
                    }
                    self.last_ms = Some(timestamp_ms);
                }
                if self.running {
                    self.clocks = self.clocks.saturating_add(1);
                }
            }
            _ => {}
        }
    }
    pub fn bpm(&self) -> Option<f64> {
        self.interval_ms
            .filter(|v| *v > 0.)
            .map(|v| 60000. / (24. * v))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_cc_and_channels() {
        assert_eq!(
            decode(&[0xb1, 25, 127], 1),
            Some(MidiEvent::Control(Automation {
                tick: 0,
                track: 1,
                controller: 3,
                value: 127
            }))
        );
        assert_eq!(decode(&[0xb1, 25, 127], 0), None);
        assert_eq!(
            decode(&[0xb0, 83, 64], 0),
            Some(MidiEvent::Control(Automation {
                tick: 0,
                track: 4,
                controller: 22,
                value: 64
            }))
        );
        assert_eq!(
            decode(&[0xb0, 94, 127], 0),
            Some(MidiEvent::Control(Automation {
                tick: 0,
                track: 7,
                controller: 4,
                value: 54
            }))
        );
    }
    #[test]
    fn keyboard_ranges_and_note_off() {
        assert_eq!(pattern_key(12), Some(PatternKey::Toggle(0)));
        assert_eq!(pattern_key(63), Some(PatternKey::Pattern(3, 7)));
        assert_eq!(panel_key(95), Some(KeyAction::ProgramSynthMode));
        assert_eq!(synth_key(32), Some(SynthKey::PitchMode));
        assert_eq!(
            decode(&[0x90, 60, 0], 0),
            Some(MidiEvent::NoteOff { note: 60 })
        );
        assert!(decode(&[0xb0, 25], 0).is_none());
    }
    #[test]
    fn sync_messages_and_tempo() {
        let mut c = MidiClock::default();
        c.process(&MidiEvent::Start, 0.);
        for i in 0..25 {
            c.process(&MidiEvent::Clock, i as f64 * 60000. / (120. * 24.));
        }
        assert!((c.bpm().unwrap() - 120.).abs() < 0.01);
        assert_eq!(c.clocks, 25);
        c.process(&MidiEvent::SongPosition { sixteenths: 32 }, 0.);
        assert_eq!(c.clocks, 192);
        c.process(&MidiEvent::Stop, 0.);
        assert!(!c.running);
        c.process(&MidiEvent::Continue, 0.);
        assert!(c.running);
    }
}
