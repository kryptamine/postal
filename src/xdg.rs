use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
#[error("neither {variable} nor HOME is set")]
pub struct NoHome {
    variable: &'static str,
}

/// `$XDG_CONFIG_HOME/postal`, falling back to `~/.config/postal` on macOS too.
pub fn config_dir() -> Result<PathBuf, NoHome> {
    postal_dir("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME/postal`, falling back to `~/.local/state/postal` on macOS too.
pub fn state_dir() -> Result<PathBuf, NoHome> {
    postal_dir("XDG_STATE_HOME", ".local/state")
}

/// `$XDG_CACHE_HOME/postal`, falling back to `~/.cache/postal` on macOS too.
pub fn cache_dir() -> Result<PathBuf, NoHome> {
    postal_dir("XDG_CACHE_HOME", ".cache")
}

/// `$XDG_DATA_HOME/postal`, falling back to `~/.local/share/postal` on macOS too.
pub fn data_dir() -> Result<PathBuf, NoHome> {
    postal_dir("XDG_DATA_HOME", ".local/share")
}

/// A relative XDG variable is ignored, as the XDG spec requires.
fn postal_dir(variable: &'static str, under_home: &str) -> Result<PathBuf, NoHome> {
    let base = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(under_home)))
        .ok_or(NoHome { variable })?;
    Ok(base.join("postal"))
}
