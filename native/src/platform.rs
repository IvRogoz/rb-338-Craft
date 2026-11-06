use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
use windows_sys::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

fn dialog(
    save: bool,
    default_name: &str,
    filter: &str,
    extension: &str,
) -> Result<Option<PathBuf>, String> {
    let mut file = vec![0u16; 32768];
    let name: Vec<u16> = default_name.encode_utf16().take(file.len() - 1).collect();
    file[..name.len()].copy_from_slice(&name);
    let filter: Vec<u16> = filter.encode_utf16().collect();
    let extension: Vec<u16> = extension.encode_utf16().chain(Some(0)).collect();
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.nFilterIndex = 1;
    ofn.lpstrFile = file.as_mut_ptr();
    ofn.nMaxFile = file.len() as u32;
    ofn.lpstrDefExt = extension.as_ptr();
    ofn.Flags = OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let ok = unsafe {
        if save {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        }
    };
    if ok == 0 {
        let error = unsafe { CommDlgExtendedError() };
        return if error == 0 {
            Ok(None)
        } else {
            Err(format!("Windows file dialog failed: {error:#x}"))
        };
    }
    let end = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Ok(Some(PathBuf::from(OsString::from_wide(&file[..end]))))
}

pub fn open_song_dialog() -> Result<Option<PathBuf>, String> {
    dialog(
        false,
        "",
        "RB-338 songs (*.rbs;*.json)\0*.rbs;*.json\0All files (*.*)\0*.*\0\0",
        "json",
    )
}

pub fn save_song_dialog(default_name: &str) -> Result<Option<PathBuf>, String> {
    dialog(
        true,
        default_name,
        "JSON songs (*.json)\0*.json\0\0",
        "json",
    )
}

pub fn save_wav_dialog(default_name: &str) -> Result<Option<PathBuf>, String> {
    dialog(true, default_name, "WAV audio (*.wav)\0*.wav\0\0", "wav")
}

pub fn library_dir() -> Result<PathBuf, String> {
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?)
        .join("RB338Craft")
        .join("Songs");
    std::fs::create_dir_all(&path).map_err(|e| format!("Cannot create song library: {e}"))?;
    Ok(path)
}

pub fn library_songs() -> Result<Vec<PathBuf>, String> {
    let mut songs = Vec::new();
    for entry in
        std::fs::read_dir(library_dir()?).map_err(|e| format!("Cannot read song library: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Cannot read song library entry: {e}"))?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|e| format!("Cannot inspect song library entry: {e}"))?
            .is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            songs.push(path);
        }
    }
    songs.sort();
    Ok(songs)
}

fn song_filename(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim_matches(|c: char| c == ' ' || c == '.');
    let clean = if clean.is_empty() { "Untitled" } else { clean };
    let stem = clean
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        || matches!(
            stem.as_str(),
            "COM¹" | "COM²" | "COM³" | "LPT¹" | "LPT²" | "LPT³"
        );
    let mut result = if reserved {
        format!("_{clean}")
    } else {
        clean.to_owned()
    };
    if !result.to_ascii_lowercase().ends_with(".json") {
        result.push_str(".json");
    }
    result
}

pub fn library_path(name: &str) -> Result<PathBuf, String> {
    Ok(library_dir()?.join(song_filename(name)))
}

#[cfg(test)]
mod tests {
    use super::song_filename;
    #[test]
    fn sanitizes_windows_filenames() {
        assert_eq!(
            song_filename("../folder\\song:take?"),
            "_folder_song_take_.json"
        );
        assert_eq!(song_filename("CON.json"), "_CON.json");
        assert_eq!(song_filename("lpt1"), "_lpt1.json");
        assert_eq!(song_filename("COM¹.txt"), "_COM¹.txt.json");
        assert_eq!(song_filename(" ... "), "Untitled.json");
        assert_eq!(song_filename("Track.JSON"), "Track.JSON");
    }
}
