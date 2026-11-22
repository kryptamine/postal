use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

/// Writes `bytes` to a file of its own in `dir`, named `name` as mail
/// gave it: a taken name becomes `name (1).pdf`, and an existing file is
/// never written over. Returns where.
pub fn write_new(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = file_name(name);
    let mut taken = 0;
    loop {
        let path = dir.join(numbered(&name, taken));
        let created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&path);
        match created {
            Ok(mut file) => {
                file.write_all(bytes)?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => taken += 1,
            Err(error) => return Err(error),
        }
    }
}

/// The name mail gave, kept inside the directory: a path separator
/// becomes `_`, and leading dots go, so it is neither hidden nor `..`.
fn file_name(sent: &str) -> String {
    let name: String = sent
        .trim()
        .trim_start_matches('.')
        .chars()
        .map(|c| if matches!(c, '/' | '\\') { '_' } else { c })
        .collect();
    if name.is_empty() {
        "attachment".to_owned()
    } else {
        name
    }
}

fn numbered(name: &str, taken: usize) -> String {
    if taken == 0 {
        return name.to_owned();
    }

    match name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()) {
        Some((stem, extension)) => format!("{stem} ({taken}).{extension}"),
        None => format!("{name} ({taken})"),
    }
}
