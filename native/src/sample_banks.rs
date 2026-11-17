//! Native song loading, including the sample bank named by an RBS file.
use rb_model::Song;
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_SONG_BYTES: usize = 16 * 1024 * 1024;
const MAX_BANK_BYTES: usize = 64 * 1024 * 1024;

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > limit as u64 {
        return Err(format!(
            "{} exceeds the {} MiB limit",
            path.display(),
            limit / 1024 / 1024
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("{} is too large", path.display()));
    }
    Ok(bytes)
}

pub fn load_path(path: &Path) -> Result<Song, String> {
    let mut roots = Vec::new();
    if let Some(folder) = std::env::var_os("RB338_MOD_DIR") {
        roots.push(PathBuf::from(folder));
    }
    if let Some(parent) = path.parent() {
        roots.push(parent.to_owned());
        for ancestor in parent.ancestors().take(3) {
            roots.push(ancestor.join("Mods"));
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.join("Mods"));
            roots.push(parent.to_owned());
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("RB338Craft/Mods"));
    }
    load_from_dirs(path, &roots)
}

pub fn load_from_dirs(path: &Path, roots: &[PathBuf]) -> Result<Song, String> {
    let data = read_bounded(path, MAX_SONG_BYTES)?;
    let mut song = if data.starts_with(b"CAT ") {
        rb_model::rbs::import(&data)?
    } else {
        let text = std::str::from_utf8(&data).map_err(|e| e.to_string())?;
        Song::from_json(text).or_else(|_| rb_model::rbs::import_legacy(text))?
    };
    song.validate()?;
    if song.loaded_bank.is_some()
        || song.sample_bank.trim().is_empty()
        || song
            .sample_bank
            .trim()
            .eq_ignore_ascii_case("Standard ReBirth")
    {
        return Ok(song);
    }
    let wanted = song.sample_bank.trim();
    let mut seen = HashSet::new();
    for root in roots {
        if !seen.insert(root.clone()) {
            continue;
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Cannot read sample-bank folder {}: {error}",
                    root.display()
                ))
            }
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let candidate = entry.path();
            if entry.file_type().map_err(|e| e.to_string())?.is_file()
                && candidate
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("rbm"))
            {
                files.push(candidate);
            }
        }
        files.sort();
        for candidate in files {
            let expected_filename = candidate
                .file_stem()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(wanted));
            let bank_bytes = match read_bounded(&candidate, MAX_BANK_BYTES) {
                Ok(bytes) => bytes,
                Err(error) if expected_filename => return Err(error),
                Err(_) => continue,
            };
            let name = match crate::rbm::bank_name(&bank_bytes) {
                Ok(name) => name,
                Err(error)
                    if candidate
                        .file_stem()
                        .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(wanted)) =>
                {
                    return Err(error)
                }
                Err(_) => continue,
            };
            if !name.trim().eq_ignore_ascii_case(wanted) {
                continue;
            }
            let bank = match crate::rbm::decode(&bank_bytes, None) {
                Ok(bank) => bank,
                Err(error) if error.contains("RB20FUL.DAT") => {
                    let dat = std::env::var_os("RB338_RB20FUL_DAT")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| candidate.with_file_name("RB20FUL.DAT"));
                    if !dat.is_file() {
                        return Err(format!("Bank {wanted:?} needs RB20FUL.DAT beside {} or RB338_RB20FUL_DAT set. {error}", candidate.display()));
                    }
                    crate::rbm::decode(&bank_bytes, Some(&read_bounded(&dat, 134_217_984)?))?
                }
                Err(error) => return Err(format!("Could not load bank {wanted:?}: {error}")),
            };
            song.install_sample_bank(bank)?;
            song.validate()?;
            return Ok(song);
        }
    }
    Err(format!("Song requires sample bank {wanted:?}. Put its .rbm file in Mods beside the song or application, or set RB338_MOD_DIR. No replacement instruments were loaded."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn referenced_bank_is_required_and_embedded_json_is_portable() {
        let root = std::env::temp_dir().join(format!("rb338-bank-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("song.json");
        let mut song = Song::default();
        song.sample_bank = "Missing bank".into();
        std::fs::write(&path, serde_json::to_vec(&song).unwrap()).unwrap();
        let error = load_from_dirs(&path, &[]).unwrap_err();
        assert!(error.contains("Missing bank"));
        let bank = rb_model::samples::SampleBank {
            name: "Missing bank".into(),
            samples: vec![None; 46],
        };
        song.install_sample_bank(bank).unwrap();
        std::fs::write(&path, serde_json::to_vec(&song).unwrap()).unwrap();
        assert!(load_from_dirs(&path, &[]).unwrap().loaded_bank.is_some());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
