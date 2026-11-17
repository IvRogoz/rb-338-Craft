//! RBM container and sample decoding derived from OpenRB-338 (GPL-3.0),
//! revision cbca4492c0224dffaa477346bc8860559f3d853f, crates/rbm.
use rb_model::samples::{CustomSample, SampleBank};
use std::sync::Arc;

const WINDOW: usize = 118_280_160;
const KEY: &[u8; 2047] = include_bytes!("rbm_key_window.bin");
const DAT_SHA: [u8; 32] = [
    0x62, 0x07, 0x1e, 0x99, 0x96, 0x97, 0xe7, 0xac, 0x49, 0x93, 0x6c, 0xf6, 0x05, 0xeb, 0x2c, 0x88,
    0x8d, 0x10, 0x97, 0xc6, 0xc6, 0xb8, 0x5e, 0xa1, 0x0c, 0x30, 0x42, 0x9a, 0x48, 0x2f, 0x27, 0x97,
];
const NAMES: [&str; 46] = [
    "TR808BD.aif",
    "TR808SD1.aif",
    "TR808SD2.aif",
    "TR808SD3.aif",
    "TR808SD4.aif",
    "TR808SD5.aif",
    "TR808N1.aif",
    "TR808N2.aif",
    "TR808N3.aif",
    "TR808N4.aif",
    "TR808CB.aif",
    "TR808CH.aif",
    "TR808CL.aif",
    "TR808CP.aif",
    "TR808CY.aif",
    "TR808HC.aif",
    "TR808HT.aif",
    "TR808LC.aif",
    "TR808LT.aif",
    "TR808MA.aif",
    "TR808MC.aif",
    "TR808MT.aif",
    "TR808OH.aif",
    "TR808RS.aif",
    "Startup.aif",
    "TR909bd1.aif",
    "TR909bd2.aif",
    "TR909bd3.aif",
    "TR909bd4.aif",
    "TR909sd1.aif",
    "TR909sd2.aif",
    "TR909sd3.aif",
    "TR909sd4.aif",
    "TR909sd5.aif",
    "TR909sdn1.aif",
    "TR909sdn2.aif",
    "TR909sdn3.aif",
    "TR909lt.aif",
    "TR909mt.aif",
    "TR909ht.aif",
    "TR909rs.aif",
    "TR909hc.aif",
    "TR909chh.aif",
    "TR909ohh.aif",
    "TR909ccy.aif",
    "TR909rcy.aif",
];
fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes(b[..4].try_into().unwrap()) as usize
}
struct Chunk<'a> {
    tag: &'a [u8],
    body: &'a [u8],
    ordinal: usize,
}
fn chunks<'a>(bytes: &'a [u8], kind: &[u8; 4], form: &[u8; 4]) -> Result<Vec<Chunk<'a>>, String> {
    if bytes.len() < 12
        || &bytes[..4] != kind
        || &bytes[8..12] != form
        || be32(&bytes[4..8]).checked_add(8) != Some(bytes.len())
    {
        return Err("Invalid container header or size".into());
    }
    let mut out = Vec::new();
    let mut p = 12usize;
    while p < bytes.len() {
        let h = bytes
            .get(p..p.saturating_add(8))
            .ok_or("Truncated chunk header")?;
        let size = be32(&h[4..]);
        let start = p + 8;
        let end = start
            .checked_add(size)
            .filter(|&n| n <= bytes.len())
            .ok_or("Invalid chunk size")?;
        out.push(Chunk {
            tag: &h[..4],
            body: &bytes[start..end],
            ordinal: out.len(),
        });
        p = end
            .checked_add(size & 1)
            .filter(|&n| n <= bytes.len())
            .ok_or("Missing chunk padding")?;
    }
    Ok(out)
}
fn name(b: &[u8]) -> String {
    b.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as char)
        .collect()
}
fn bank_info(chunks: &[Chunk<'_>]) -> Result<String, String> {
    let info = chunks
        .iter()
        .find(|c| c.tag == b"INFO" && c.body.len() == 1280)
        .ok_or("Missing RBM INFO")?;
    let result = name(&info.body[1003..1067]);
    if result.trim().is_empty() {
        return Err("Empty RBM name".into());
    }
    Ok(result)
}
pub fn bank_name(bytes: &[u8]) -> Result<String, String> {
    bank_info(&chunks(bytes, b"CAT ", b"PRBM")?)
}
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptOpenAlgorithmProvider(
        h: *mut *mut std::ffi::c_void,
        id: *const u16,
        implementation: *const u16,
        flags: u32,
    ) -> i32;
    fn BCryptHash(
        h: *mut std::ffi::c_void,
        secret: *const u8,
        secret_len: u32,
        input: *const u8,
        input_len: u32,
        output: *mut u8,
        output_len: u32,
    ) -> i32;
    fn BCryptCloseAlgorithmProvider(h: *mut std::ffi::c_void, flags: u32) -> i32;
}
fn valid_dat(dat: &[u8]) -> Result<(), String> {
    if dat.len() != 134_217_984 {
        return Err("RB20FUL.DAT has the wrong size".into());
    }
    let mut h = std::ptr::null_mut();
    let mut digest = [0u8; 32];
    let id: Vec<u16> = "SHA256\0".encode_utf16().collect();
    unsafe {
        if BCryptOpenAlgorithmProvider(&mut h, id.as_ptr(), std::ptr::null(), 0) < 0 {
            return Err("SHA-256 unavailable".into());
        }
        let result = BCryptHash(
            h,
            std::ptr::null(),
            0,
            dat.as_ptr(),
            dat.len() as u32,
            digest.as_mut_ptr(),
            32,
        );
        BCryptCloseAlgorithmProvider(h, 0);
        if result < 0 || digest != DAT_SHA {
            return Err("RB20FUL.DAT SHA-256 does not match the supported edition".into());
        }
    }
    Ok(())
}
fn aiff(data: &[u8]) -> Result<Vec<i16>, String> {
    let cs = chunks(data, b"FORM", b"AIFF")?;
    let comm = cs
        .iter()
        .find(|c| c.tag == b"COMM")
        .ok_or("Missing AIFF COMM")?
        .body;
    let snd = cs
        .iter()
        .find(|c| c.tag == b"SSND")
        .ok_or("Missing AIFF SSND")?
        .body;
    if comm.len() < 18 || snd.len() < 8 {
        return Err("Truncated AIFF metadata".into());
    }
    let channels = u16::from_be_bytes([comm[0], comm[1]]) as usize;
    let frames = be32(&comm[2..6]);
    let bits = u16::from_be_bytes([comm[6], comm[7]]);
    if channels == 0 || channels > 32 || bits != 16 || frames == 0 || frames > 2_000_000 {
        return Err("Unsupported AIFF format".into());
    }
    let begin = 8usize
        .checked_add(be32(&snd[..4]))
        .ok_or("Invalid AIFF offset")?;
    let need = frames
        .checked_mul(channels)
        .and_then(|n| n.checked_mul(2))
        .ok_or("Invalid AIFF frame count")?;
    let audio = snd
        .get(begin..begin.checked_add(need).ok_or("Invalid AIFF size")?)
        .ok_or("Truncated AIFF audio")?;
    Ok(audio
        .chunks_exact(channels * 2)
        .map(|frame| i16::from_be_bytes([frame[0], frame[1]]))
        .collect())
}
pub fn decode(bytes: &[u8], dat: Option<&[u8]>) -> Result<SampleBank, String> {
    let cs = chunks(bytes, b"CAT ", b"PRBM")?;
    let bank = bank_info(&cs)?;
    let mut sums = [0x2000u32, 0x3000u32];
    for c in cs.iter().filter(|c| c.tag == b"EMBF") {
        if !c.body.contains(&0) {
            return Err("RBM file name is not terminated".into());
        }
        let mut b = [0x65u8; 1024];
        let n = c.body.len().min(1024);
        b[..n].copy_from_slice(&c.body[..n]);
        for k in 0..2 {
            for w in b[k * 512..k * 512 + 512].chunks_exact(4) {
                sums[k] = sums[k].wrapping_add(be32(w) as u32);
            }
        }
    }
    let pages = sums.map(|s| 256 + (s as usize) % (0x800_0000 - 2048));
    let needs_dat = cs
        .iter()
        .filter(|c| c.tag == b"EMBF" && c.body.len() > 1024)
        .any(|c| {
            NAMES
                .iter()
                .enumerate()
                .any(|(i, n)| i != 24 && name(c.body).eq_ignore_ascii_case(n))
                && pages[(c.ordinal as u8 as usize) & 1] != WINDOW
        });
    if needs_dat {
        valid_dat(dat.ok_or("This RBM requires RB20FUL.DAT")?)?;
    }
    let mut samples = vec![None; 46];
    let mut total = 0usize;
    for (i, slot) in NAMES.iter().enumerate() {
        if i == 24 {
            continue;
        }
        let Some(c) = cs
            .iter()
            .find(|c| c.tag == b"EMBF" && name(c.body).eq_ignore_ascii_case(slot))
        else {
            continue;
        };
        let nul = c
            .body
            .iter()
            .position(|&b| b == 0)
            .ok_or("Unterminated RBM name")?;
        let key = c.ordinal as u8 as usize;
        let page = key & 1;
        let period = (17 * key) % 1925 + 123;
        let mut payload = c.body.to_vec();
        if payload.len() > 1024 {
            let table: &[u8] = if pages[page] == WINDOW {
                KEY
            } else {
                let d = dat.ok_or("This RBM requires RB20FUL.DAT")?;
                &d[pages[page]..pages[page] + 2048]
            };
            for (n, b) in payload.iter_mut().skip(1024).enumerate() {
                *b = b.wrapping_add(table[n % period]);
            }
        }
        let pcm = aiff(&payload[nul + 1..]).map_err(|e| format!("{slot}: {e}"))?;
        total = total.checked_add(pcm.len()).ok_or("Bank too large")?;
        if total > 2_000_000 {
            return Err("RBM exceeds 2,000,000 sample frames".into());
        }
        samples[i] = Some(Arc::new(CustomSample {
            name: (*slot).into(),
            sample_rate: 44100,
            pcm,
        }));
    }
    Ok(SampleBank {
        name: bank,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = tag.to_vec();
        b.extend((body.len() as u32).to_be_bytes());
        b.extend(body);
        if body.len() % 2 != 0 {
            b.push(0)
        }
        b
    }
    fn container(form: &[u8; 4], items: &[Vec<u8>], kind: &[u8; 4]) -> Vec<u8> {
        let mut b = kind.to_vec();
        let n = 4 + items.iter().map(Vec::len).sum::<usize>();
        b.extend((n as u32).to_be_bytes());
        b.extend(form);
        for item in items {
            b.extend(item)
        }
        b
    }
    fn fixture() -> Vec<u8> {
        let mut info = vec![0; 1280];
        info[1003..1007].copy_from_slice(b"Test");
        let mut comm = vec![0; 18];
        comm[1] = 1;
        comm[2..6].copy_from_slice(&700u32.to_be_bytes());
        comm[7] = 16;
        comm[8..18].copy_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);
        let mut sound = vec![0; 8];
        for i in 0..700i16 {
            sound.extend(i.to_be_bytes());
        }
        let aiff = container(
            b"AIFF",
            &[chunk(b"COMM", &comm), chunk(b"SSND", &sound)],
            b"FORM",
        );
        let mut sample = b"TR909ccy.aif\0".to_vec();
        sample.extend(aiff);
        let mut pad = vec![0; 1024];
        pad[..4].copy_from_slice(b"pad\0");
        let mut sums = [0x2000u32, 0x3000u32];
        for p in [&sample[..], &pad[..]] {
            let mut x = [0x65; 1024];
            let n = p.len().min(1024);
            x[..n].copy_from_slice(&p[..n]);
            for k in 0..2 {
                for w in x[k * 512..k * 512 + 512].chunks_exact(4) {
                    sums[k] = sums[k].wrapping_add(be32(w) as u32);
                }
            }
        }
        for k in 0..2 {
            let delta = (WINDOW as u32 - 256).wrapping_sub(sums[k]);
            let pos = 100 + k * 512;
            pad[pos..pos + 4].copy_from_slice(&delta.to_be_bytes());
        }
        // INFO ordinal 0, pad 1, sample 2: scramble with embedded page.
        let period = (17 * 2) % 1925 + 123;
        for (n, b) in sample.iter_mut().skip(1024).enumerate() {
            *b = b.wrapping_sub(KEY[n % period]);
        }
        container(
            b"PRBM",
            &[
                chunk(b"INFO", &info),
                chunk(b"EMBF", &pad),
                chunk(b"EMBF", &sample),
            ],
            b"CAT ",
        )
    }
    #[test]
    fn recrypted_bank_decodes_and_checks_bounds() {
        let b = fixture();
        assert_eq!(bank_name(&b).unwrap(), "Test");
        let s = decode(&b, None).unwrap();
        let pcm = &s.samples[44].as_ref().unwrap().pcm;
        assert_eq!(pcm.len(), 700);
        assert_eq!(pcm[699], 699);
        let mut broken = b.clone();
        broken[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&broken, None).is_err());
        assert!(decode(&b[..b.len() - 1], None).is_err());
    }
    #[test]
    fn rejects_bad_dat() {
        assert!(valid_dat(b"bad").is_err());
    }
}
