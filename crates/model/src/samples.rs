use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const MAX_WAV_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SAMPLE_FRAMES: usize = 2_000_000;
pub type SampleSlots = [[Option<Arc<CustomSample>>; 12]; 2];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CustomSample {
    pub name: String,
    pub sample_rate: u32,
    pub pcm: Vec<i16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SampleBank {
    pub name: String,
    /// Resource IDs 1000..=1045, including the reserved slot at 1000.
    pub samples: Vec<Option<Arc<CustomSample>>>,
}

pub fn deserialize_slots<'de, D: serde::Deserializer<'de>>(d: D) -> Result<SampleSlots, D::Error> {
    let slots = SampleSlots::deserialize(d)?;
    let mut total = 0usize;
    for sample in slots.iter().flatten().flatten() {
        total = total
            .checked_add(sample.pcm.len())
            .ok_or_else(|| serde::de::Error::custom("sample frame limit exceeded"))?;
        if total > MAX_SAMPLE_FRAMES
            || sample.pcm.is_empty()
            || !(8000..=192000).contains(&sample.sample_rate)
        {
            return Err(serde::de::Error::custom(
                "invalid sample or sample frame limit exceeded",
            ));
        }
    }
    Ok(slots)
}

impl CustomSample {
    pub fn from_wav(name: String, wav: &[u8]) -> Result<Self, String> {
        fn u16le(x: &[u8]) -> u16 {
            u16::from_le_bytes([x[0], x[1]])
        }
        fn u32le(x: &[u8]) -> u32 {
            u32::from_le_bytes([x[0], x[1], x[2], x[3]])
        }
        if wav.len() > MAX_WAV_BYTES {
            return Err("WAV file exceeds 32 MiB".into());
        }
        if wav.len() < 12 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
            return Err("Expected a RIFF WAVE file".into());
        }
        let end = (u32le(&wav[4..8]) as usize)
            .checked_add(8)
            .ok_or("Invalid WAV size")?;
        if end > wav.len() || end < 12 {
            return Err("Truncated WAV file".into());
        }
        let mut format = None;
        let mut data = None;
        let mut pos = 12;
        while pos < end {
            if end - pos < 8 {
                return Err("Truncated WAV chunk".into());
            }
            let size = u32le(&wav[pos + 4..pos + 8]) as usize;
            let start = pos + 8;
            let stop = start.checked_add(size).ok_or("Invalid WAV chunk size")?;
            if stop > end {
                return Err("Truncated WAV chunk".into());
            }
            if &wav[pos..pos + 4] == b"fmt " {
                format = Some(&wav[start..stop]);
            }
            if &wav[pos..pos + 4] == b"data" {
                data = Some(&wav[start..stop]);
            }
            pos = stop.checked_add(size & 1).ok_or("Invalid WAV padding")?;
            if pos > end {
                return Err("Truncated WAV padding".into());
            }
        }
        let fmt = format.ok_or("Missing WAV format")?;
        let data = data.ok_or("Missing WAV data")?;
        if fmt.len() < 16 {
            return Err("Truncated WAV format".into());
        }
        let encoding = u16le(fmt);
        let channels = u16le(&fmt[2..]) as usize;
        let sample_rate = u32le(&fmt[4..]);
        let alignment = u16le(&fmt[12..]) as usize;
        let bits = u16le(&fmt[14..]);
        if !(1..=2).contains(&channels)
            || !(8000..=192000).contains(&sample_rate)
            || !matches!((encoding, bits), (1, 8 | 16 | 24 | 32) | (3, 32))
        {
            return Err(
                "WAV must be mono/stereo PCM 8/16/24/32-bit or float32 at 8–192 kHz".into(),
            );
        }
        let bytes = bits as usize / 8;
        if alignment != channels * bytes || data.len() % alignment != 0 {
            return Err("Invalid WAV frame alignment".into());
        }
        let frames = data.len() / alignment;
        if frames == 0 || frames > MAX_SAMPLE_FRAMES {
            return Err("WAV sample frame limit exceeded".into());
        }
        let mut pcm = Vec::with_capacity(frames);
        for frame in data.chunks_exact(alignment) {
            let mut sum = 0f64;
            for ch in frame.chunks_exact(bytes) {
                let x = match (encoding, bits) {
                    (1, 8) => (ch[0] as f64 - 128.) / 128.,
                    (1, 16) => i16::from_le_bytes([ch[0], ch[1]]) as f64 / 32768.,
                    (1, 24) => {
                        (i32::from_le_bytes([0, ch[0], ch[1], ch[2]]) >> 8) as f64 / 8388608.
                    }
                    (1, 32) => i32::from_le_bytes(ch.try_into().unwrap()) as f64 / 2147483648.,
                    (3, 32) => f32::from_le_bytes(ch.try_into().unwrap()) as f64,
                    _ => unreachable!(),
                };
                if !x.is_finite() {
                    return Err("WAV contains a non-finite sample".into());
                }
                sum += x;
            }
            pcm.push(
                (sum / channels as f64 * 32768.)
                    .round()
                    .clamp(-32768., 32767.) as i16,
            );
        }
        Ok(Self {
            name,
            sample_rate,
            pcm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wav(channels: u16, rate: u32, bits: u16, format: u16, data: &[u8]) -> Vec<u8> {
        let mut x = Vec::new();
        x.extend(b"RIFF");
        x.extend(((36 + data.len()) as u32).to_le_bytes());
        x.extend(b"WAVEfmt ");
        x.extend(16u32.to_le_bytes());
        x.extend(format.to_le_bytes());
        x.extend(channels.to_le_bytes());
        x.extend(rate.to_le_bytes());
        let align = channels * bits / 8;
        x.extend((rate * align as u32).to_le_bytes());
        x.extend(align.to_le_bytes());
        x.extend(bits.to_le_bytes());
        x.extend(b"data");
        x.extend((data.len() as u32).to_le_bytes());
        x.extend(data);
        x
    }
    #[test]
    fn stereo_pcm_roundtrip_and_malformed_rejected() {
        let file = wav(2, 22050, 16, 1, &[0, 64, 0, 192, 255, 127, 255, 127]);
        let sample = CustomSample::from_wav("test".into(), &file).unwrap();
        assert_eq!(sample.pcm, [0, 32767]);
        assert_eq!(sample.sample_rate, 22050);
        assert!(CustomSample::from_wav("x".into(), &file[..file.len() - 1]).is_err());
        let mut song = crate::Song::default();
        song.load_sample(0, 1, "test".into(), &file).unwrap();
        let json = serde_json::to_vec(&song).unwrap();
        let loaded: crate::Song = serde_json::from_slice(&json).unwrap();
        assert_eq!(song.samples, loaded.samples);
        let mut old = serde_json::to_value(&song).unwrap();
        old.as_object_mut().unwrap().remove("samples");
        let old: crate::Song = serde_json::from_value(old).unwrap();
        assert!(old.samples.iter().flatten().all(Option::is_none));
        let pcm24 = wav(1, 44100, 24, 1, &[0, 0, 128, 255, 255, 127]);
        assert_eq!(
            CustomSample::from_wav("24-bit".into(), &pcm24).unwrap().pcm,
            [-32768, 32767]
        );
        let float = wav(1, 44100, 32, 3, &0.5f32.to_le_bytes());
        assert_eq!(
            CustomSample::from_wav("float".into(), &float).unwrap().pcm,
            [16384]
        );
        song.clear_sample(0, 1).unwrap();
        assert!(song.samples[0][1].is_none());
        let invalid = wav(1, 44100, 32, 3, &f32::NAN.to_le_bytes());
        assert!(CustomSample::from_wav("nan".into(), &invalid).is_err());
    }
    #[test]
    fn maximum_pcm_fits_compact_song_and_oversize_is_rejected() {
        let mut song = crate::Song::default();
        song.samples[0][1] = Some(Arc::new(CustomSample {
            name: "maximum".into(),
            sample_rate: 44100,
            pcm: vec![-32768; MAX_SAMPLE_FRAMES],
        }));
        let json = serde_json::to_vec(&song).unwrap();
        assert!(json.len() < 16 * 1024 * 1024);
        let loaded: crate::Song = serde_json::from_slice(&json).unwrap();
        assert_eq!(
            loaded.samples[0][1].as_ref().unwrap().pcm.len(),
            MAX_SAMPLE_FRAMES
        );
        song.samples[1][1] = Some(Arc::new(CustomSample {
            name: "extra".into(),
            sample_rate: 44100,
            pcm: vec![0],
        }));
        assert!(song.validate().is_err());
        assert!(
            serde_json::from_slice::<crate::Song>(&serde_json::to_vec(&song).unwrap()).is_err()
        );
    }
    #[test]
    fn oversized_song_rejects_sample_and_restores_previous_assignment() {
        let mut song = crate::Song::default();
        let original = Arc::new(CustomSample {
            name: "old".into(),
            sample_rate: 8000,
            pcm: vec![42],
        });
        song.samples[0][1] = Some(original.clone());
        song.name = "x".repeat(16 * 1024 * 1024);
        let file = wav(1, 8000, 16, 1, &[0, 64]);
        let error = song.load_sample(0, 1, "new".into(), &file).unwrap_err();
        assert!(error.contains("16 MiB"));
        assert!(Arc::ptr_eq(song.samples[0][1].as_ref().unwrap(), &original));
    }
    #[test]
    fn bank_install_persists_and_rejects_invalid_atomically() {
        let mut song = crate::Song::default();
        let mut bank = SampleBank { name:"bank".into(), samples:vec![None;46] };
        bank.samples[45] = Some(Arc::new(CustomSample { name:"rc".into(), sample_rate:44100, pcm:vec![7, -8] }));
        song.install_sample_bank(bank.clone()).unwrap();
        let json = serde_json::to_string(&song).unwrap();
        let loaded = crate::Song::from_json(&json).unwrap();
        assert_eq!(loaded.loaded_bank.as_deref(),Some(&bank));
        let old = song.loaded_bank.clone().unwrap();
        bank.samples.pop();
        assert!(song.install_sample_bank(bank).is_err());
        assert!(Arc::ptr_eq(song.loaded_bank.as_ref().unwrap(),&old));
    }

    #[test]
    fn loaded_bank_must_match_nonempty_reference() {
        let mut song = crate::Song::default();
        song.sample_bank = "Trancer".into();
        let mut bank = SampleBank { name:"trancer".into(), samples:vec![None;46] };
        song.install_sample_bank(bank.clone()).unwrap();
        assert!(song.validate().is_ok());
        let old=song.loaded_bank.clone().unwrap();
        bank.name="other".into();
        assert!(song.install_sample_bank(bank.clone()).is_err());
        assert!(Arc::ptr_eq(song.loaded_bank.as_ref().unwrap(), &old));
        song.loaded_bank=Some(Arc::new(bank));
        assert!(song.validate().is_err());
        assert!(crate::Song::from_json(&serde_json::to_string(&song).unwrap()).is_err());
        song.loaded_bank=None;
        assert!(song.validate().is_ok());
    }

}
