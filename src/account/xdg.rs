use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[error("HOME is not set{}", .variable.map(|v| format!(", nor {v}")).unwrap_or_default())]
pub struct NoHome {
    /// The XDG variable that would have stood in for it.
    variable: Option<&'static str>,
}

/// `$HOME`.
pub fn home_dir() -> Result<PathBuf, NoHome> {
    let home = std::env::var_os("HOME").ok_or(NoHome { variable: None })?;
    Ok(PathBuf::from(home))
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

/// `$XDG_DATA_HOME/applications`, where desktop entries of the user's own go.
pub fn applications_dir() -> Result<PathBuf, NoHome> {
    Ok(base_dir("XDG_DATA_HOME", ".local/share")?.join("applications"))
}

/// `$XDG_DOWNLOAD_DIR`, falling back to `~/Downloads`: the user's, not
/// Postal's own.
pub fn download_dir() -> Result<PathBuf, NoHome> {
    base_dir("XDG_DOWNLOAD_DIR", "Downloads")
}

fn postal_dir(variable: &'static str, under_home: &str) -> Result<PathBuf, NoHome> {
    Ok(base_dir(variable, under_home)?.join("postal"))
}

/// A relative XDG variable is ignored, as the XDG spec requires.
fn base_dir(variable: &'static str, under_home: &str) -> Result<PathBuf, NoHome> {
    let unset = |_unset: NoHome| NoHome {
        variable: Some(variable),
    };

    let set = std::env::var_os(variable).map(PathBuf::from);
    set.filter(|path| path.is_absolute()).map_or_else(
        || home_dir().map(|home| home.join(under_home)).map_err(unset),
        Ok,
    )
}
