use std::{
    error::Error,
    fs::{self, DirBuilder},
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::account::{atomic_file, xdg};

/// Reads the link and the path of postal as data, `"$(…)"` being one word
/// a shell never parses again, then removes its directory and runs postal.
const SCRIPT: &str = "#!/bin/sh\nd=${0%/*}\n\
    link=$(cat \"$d/link\") && postal=$(cat \"$d/postal\") || exit 1\n\
    rm -rf \"$d\"\nexec \"$postal\" \"$link\"\n";

/// Writes a script that runs `postal <link>` once, the link and the path
/// beside it in a directory of their own (ADR 0034). Returns the script.
pub fn write(link: &str, postal: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let since = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let name = format!("{}-{}", since.as_nanos(), std::process::id());
    let dir = xdg::state_dir()?.join("mailto").join(name);
    fs::create_dir_all(dir.parent().unwrap_or(&dir))?;
    DirBuilder::new().mode(0o700).create(&dir)?;
    atomic_file::write(&dir.join("link"), link.as_bytes(), 0o600)?;
    atomic_file::write(&dir.join("postal"), postal.as_os_str().as_bytes(), 0o600)?;
    let script = dir.join("postal.command");
    atomic_file::write(&script, SCRIPT.as_bytes(), 0o700)?;
    Ok(script)
}
