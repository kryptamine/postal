use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct NoHome {
    variable: &'static str,
}

impl fmt::Display for NoHome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "neither {} nor HOME is set", self.variable)
    }
}

impl std::error::Error for NoHome {}

/// `$XDG_CONFIG_HOME/postal`, falling back to `~/.config/postal` on macOS too.
pub fn config_dir() -> Result<PathBuf, NoHome> {
    postal_dir("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME/postal`, falling back to `~/.local/state/postal` on macOS too.
pub fn state_dir() -> Result<PathBuf, NoHome> {
    postal_dir("XDG_STATE_HOME", ".local/state")
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
