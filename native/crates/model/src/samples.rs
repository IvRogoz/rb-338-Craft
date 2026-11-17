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
        let mut encoding = u16le(fmt);
        let channels = u16le(&fmt[2..]) as usize;
        let sample_rate = u32le(&fmt[4..]);
        let alignment = u16le(&fmt[12..]) as usize;
        let bits = u16le(&fmt[14..]);
        if encoding == 0xfffe {
            if fmt.len() < 40 || u16le(&fmt[16..]) < 22
                || 18 + u16le(&fmt[16..]) as usize > fmt.len()
            {
                return Err("Truncated WAVEFORMATEXTENSIBLE format".into());
            }
            let subtype = u32le(&fmt[24..]);
            if !matches!(subtype, 1 | 3)
                || fmt[28..40] != [0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]
            {
                return Err("WAV extensible subtype must be PCM or float32".into());
            }
            let valid_bits = u16le(&fmt[18..]);
            if valid_bits == 0 || valid_bits > bits || (subtype == 3 && valid_bits != bits) {
                return Err("Invalid WAV valid-bit count".into());
            }
            encoding = subtype as u16;
        }
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
    fn extensible_wav(bits: u16, valid_bits: u16, format: u16, data: &[u8]) -> Vec<u8> {
        let mut file = wav(1, 44100, bits, 0xfffe, data);
        let mut extension = Vec::from(22u16.to_le_bytes());
        extension.extend(valid_bits.to_le_bytes());
        extension.extend(4u32.to_le_bytes());
        extension.extend((format as u32).to_le_bytes());
        extension.extend([0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]);
        file.splice(36..36, extension);
        file[16..20].copy_from_slice(&40u32.to_le_bytes());
        let size = (file.len() - 8) as u32;
        file[4..8].copy_from_slice(&size.to_le_bytes());
        file
    }
    #[test]
    fn extensible_pcm_and_float_samples_decode() {
        let pcm: Vec<_> = [i32::MIN, 0, 0x40000000, 0x7fffff00].into_iter().flat_map(i32::to_le_bytes).collect();
        let file = extensible_wav(32, 24, 1, &pcm);
        assert_eq!(CustomSample::from_wav("pcm".into(), &file).unwrap().pcm, [-32768, 0, 16384, 32767]);
        let floats: Vec<_> = [-0.5f32, 0., 0.5].into_iter().flat_map(f32::to_le_bytes).collect();
        let file = extensible_wav(32, 32, 3, &floats);
        assert_eq!(CustomSample::from_wav("float".into(), &file).unwrap().pcm, [-16384, 0, 16384]);
    }
    #[test]
    fn malformed_extensible_format_is_rejected() {
        let file = extensible_wav(32, 24, 1, &0i32.to_le_bytes());
        for (offset, value) in [(36, 21), (36, 24), (38, 0), (38, 33), (46, 1), (48, 1)] {
            let mut bad = file.clone();
            bad[offset] = value;
            assert!(CustomSample::from_wav("invalid".into(), &bad).is_err());
        }
        assert!(CustomSample::from_wav("short".into(), &wav(1, 44100, 32, 0xfffe, &[0; 4])).is_err());
        assert!(CustomSample::from_wav("float bits".into(), &extensible_wav(32, 24, 3, &[0; 4])).is_err());
    }
    #[test]
    fn bank_name_matching_uses_the_same_whitespace_rules_as_lookup() {
        for name in ["Trancer", " TrAnCeR "] {
            let mut song = crate::Song::default();
            song.sample_bank = name.into();
            let bank = SampleBank { name: "Trancer \t".into(), samples: vec![None; 46] };
            song.install_sample_bank(bank.clone()).unwrap();
            song.validate().unwrap();
            let restored = crate::Song::from_json(&serde_json::to_string(&song).unwrap()).unwrap();
            assert_eq!(restored.sample_bank, name);
            assert_eq!(restored.loaded_bank.as_deref(), Some(&bank));
            let previous = song.loaded_bank.clone().unwrap();
            assert!(song.install_sample_bank(SampleBank { name: "Other".into(), samples: vec![None; 46] }).is_err());
            assert!(Arc::ptr_eq(song.loaded_bank.as_ref().unwrap(), &previous));
        }
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
