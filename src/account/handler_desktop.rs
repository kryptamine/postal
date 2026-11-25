use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

use crate::account::{atomic_file, removed::removed, run::run, xdg};

pub const CHOOSES_TERMINAL: bool = false;

const MIME: &str = "x-scheme-handler/mailto";
const ENTRY: &str = "postal.desktop";

/// `$XDG_DATA_HOME/applications/postal.desktop`, the handler on Linux.
pub fn path() -> Result<PathBuf, xdg::NoHome> {
    Ok(xdg::applications_dir()?.join(ENTRY))
}

/// Writes the desktop entry that runs `postal` on a link in a terminal,
/// then makes it the default for mailto: links.
pub fn install(postal: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = path()?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Postal\n\
         Comment=Write the mail a mailto: link holds\nExec={} %u\nTerminal=true\n\
         NoDisplay=true\nMimeType={MIME};\n",
        exec_argument(postal)?
    );
    atomic_file::write(&path, entry.as_bytes(), 0o644)?;
    let mut default = Command::new("xdg-mime");
    default.args(["default", ENTRY, MIME]);
    run(default)?;
    Ok(path)
}

/// Deletes the entry; the desktop then picks the next handler it knows.
pub fn remove() -> Result<Option<PathBuf>, Box<dyn Error>> {
    let path = path()?;
    Ok(removed(&path)?.then_some(path))
}

/// `postal` quoted for `Exec`: the quoting the Desktop Entry spec asks of
/// an argument, then the escapes of any of its strings.
fn exec_argument(postal: &Path) -> Result<String, String> {
    let text = postal
        .to_str()
        .filter(|text| !text.contains(['\n', '\r']))
        .ok_or_else(|| format!("{} cannot be written in a desktop entry", postal.display()))?;
    let mut quoted = String::from("\"");
    for c in text.chars() {
        match c {
            '"' | '`' | '$' => quoted.push_str("\\\\"),
            '\\' => quoted.push_str("\\\\\\"),
            '%' => quoted.push('%'),
            _ => {}
        }

        quoted.push(c);
    }

    quoted.push('"');
    Ok(quoted)
}
