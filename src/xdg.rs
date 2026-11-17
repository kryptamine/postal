use std::{fmt, path::PathBuf};

#[derive(Debug)]
pub struct NoHome;

impl fmt::Display for NoHome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("neither XDG_CONFIG_HOME nor HOME is set")
    }
}

impl std::error::Error for NoHome {}

/// `$XDG_CONFIG_HOME/postal`, falling back to `~/.config/postal` on macOS too.
/// A relative `XDG_CONFIG_HOME` is ignored, as the XDG spec requires.
pub fn config_dir() -> Result<PathBuf, NoHome> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or(NoHome)?;
    Ok(base.join("postal"))
}
