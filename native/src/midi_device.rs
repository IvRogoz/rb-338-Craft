use crate::audio::AudioMessage;
use std::{
    mem::size_of,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
};
use windows_sys::Win32::Media::{Audio::*, MM_MIM_DATA};

pub struct MidiDevice {
    pub id: u32,
    pub name: String,
}
pub fn devices() -> Result<Vec<MidiDevice>, String> {
    let mut result = Vec::new();
    unsafe {
        for id in 0..midiInGetNumDevs() {
            let mut caps = MIDIINCAPSW::default();
            let code = midiInGetDevCapsW(id as usize, &mut caps, size_of::<MIDIINCAPSW>() as u32);
            if code != 0 {
                return Err(format!("midiInGetDevCapsW failed: {code}"));
            }
            let name = caps.szPname;
            let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            result.push(MidiDevice {
                id,
                name: String::from_utf16_lossy(&name[..end]),
            });
        }
    }
    Ok(result)
}
struct Context {
    tx: Sender<Vec<u8>>,
    audio_tx: Sender<AudioMessage>,
    sync: Arc<AtomicBool>,
}
pub struct MidiInput {
    handle: HMIDIIN,
    context: *mut Context,
}
pub fn open(
    id: u32,
    tx: Sender<Vec<u8>>,
    audio_tx: Sender<AudioMessage>,
    sync: Arc<AtomicBool>,
) -> Result<MidiInput, String> {
    let context = Box::into_raw(Box::new(Context { tx, audio_tx, sync }));
    let mut handle = ptr::null_mut();
    unsafe {
        let code = midiInOpen(
            &mut handle,
            id,
            callback as *const () as usize,
            context as usize,
            CALLBACK_FUNCTION,
        );
        if code != 0 {
            drop(Box::from_raw(context));
            return Err(format!("midiInOpen failed: {code}"));
        }
        let code = midiInStart(handle);
        if code != 0 {
            let close = midiInClose(handle);
            if close == 0 {
                drop(Box::from_raw(context));
            }
            return Err(if close == 0 {
                format!("midiInStart failed: {code}")
            } else {
                format!("midiInStart failed: {code}; midiInClose failed: {close}")
            });
        }
    }
    Ok(MidiInput { handle, context })
}
unsafe extern "system" fn callback(
    _handle: HMIDIIN,
    message: u32,
    instance: usize,
    data: usize,
    _time: usize,
) {
    if message == MM_MIM_DATA && instance != 0 {
        if let Some(bytes) = short_message(data as u32) {
            let context = &*(instance as *const Context);
            let command = match bytes[0] {
                0xf8 => Some((10, 0)),
                0xfa => Some((11, 0)),
                0xfb => Some((12, 0)),
                0xfc => Some((13, 0)),
                0xf2 => Some((14, bytes[1] as u32 | ((bytes[2] as u32) << 7))),
                _ => None,
            };
            if context.sync.load(Ordering::Relaxed) {
                if let Some((command, value)) = command {
                    let _ = context.audio_tx.send(AudioMessage::Command(command, value));
                    // The UI ends its recording punch on Stop; it does not resend transport.
                    if command == 13 {
                        let _ = context.tx.send(bytes);
                    }
                    return;
                }
            }
            let _ = context.tx.send(bytes);
        }
    }
}
fn short_message(data: u32) -> Option<Vec<u8>> {
    let status = data as u8;
    let len = match status {
        0x80..=0xbf | 0xe0..=0xef | 0xf2 => 3,
        0xc0..=0xdf | 0xf1 | 0xf3 => 2,
        0xf6..=0xff => 1,
        _ => return None,
    };
    Some((0..len).map(|i| (data >> (8 * i)) as u8).collect())
}
impl Drop for MidiInput {
    fn drop(&mut self) {
        unsafe {
            midiInStop(self.handle);
            midiInReset(self.handle);
            if midiInClose(self.handle) == 0 {
                drop(Box::from_raw(self.context));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn lengths() {
        assert_eq!(
            super::short_message(0x007f4090),
            Some(vec![0x90, 0x40, 0x7f])
        );
        assert_eq!(super::short_message(0x000012c0), Some(vec![0xc0, 0x12]));
        assert_eq!(super::short_message(0xf8), Some(vec![0xf8]));
        assert_eq!(super::short_message(0xf0), None);
    }
}
