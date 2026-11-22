use std::{
    io,
    path::{Path, PathBuf},
};

/// An Account's signature as `config.toml` gives it: the text itself, or
/// the text file holding it.
pub enum Signature {
    Text(String),
    File(PathBuf),
}

impl Signature {
    /// The file `written` names: `~` is the home directory, and a relative
    /// path is beside `config.toml`, in `dir`.
    pub fn file(dir: &Path, written: &str) -> Self {
        let home = std::env::var_os("HOME").filter(|home| !home.is_empty());
        match (written.strip_prefix("~/"), home) {
            (Some(under), Some(home)) => Self::File(Path::new(&home).join(under)),
            _ => Self::File(dir.join(written)),
        }
    }

    /// The text as it reads now, without the blank lines around it; `None`
    /// when nothing else is left.
    pub fn text(&self) -> io::Result<Option<String>> {
        let text = match self {
            Self::Text(text) => text.clone(),
            Self::File(path) => std::fs::read_to_string(path)?,
        };

        let text = text.replace("\r\n", "\n");
        let text = text.trim_matches('\n').trim_end();
        Ok((!text.is_empty()).then(|| text.to_owned()))
    }
}
