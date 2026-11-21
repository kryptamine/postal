use std::{
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

/// Replaces `path` with `contents` by a rename, so a reader sees the old file
/// or the new one, never a torn one. `mode` applies before any byte is written.
pub fn write(path: &Path, contents: &[u8], mode: u32) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".tmp");
    let temporary = dir.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
}
