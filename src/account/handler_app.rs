use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::account::{removed::removed, run::run, xdg};

pub const CHOOSES_TERMINAL: bool = true;

/// The launcher app `make launcher` builds; empty in a build without it.
const LAUNCHER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/launcher.zip"));
/// The app's name in the launcher's zip, and in `~/Applications`.
const APP: &str = "Postal.app";
const EXECUTABLE: &str = "Contents/MacOS/postal-mailto";

/// `~/Applications/Postal.app`, where the launcher goes on macOS.
pub fn path() -> Result<PathBuf, xdg::NoHome> {
    Ok(xdg::home_dir()?.join("Applications").join(APP))
}

/// Unpacks the launcher into `~/Applications`, then has it make itself the
/// handler, running `postal` for each link.
pub fn install(postal: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if LAUNCHER.is_empty() {
        return Err("built without the mailto: launcher: `make install` adds it".into());
    }

    let app = path()?;
    let dir = app.parent().ok_or("~/Applications has no parent")?;
    fs::create_dir_all(dir)?;
    let zip = dir.join(format!(".postal-launcher-{}.zip", std::process::id()));
    fs::write(&zip, LAUNCHER)?;
    removed(&app)?;
    let mut ditto = Command::new("ditto");
    ditto.args(["-x", "-k"]).arg(&zip).arg(dir);
    let unpacked = run(ditto);
    fs::remove_file(&zip)?;
    unpacked?;

    let mut register = Command::new(app.join(EXECUTABLE));
    register.arg("register").arg(postal);
    run(register)?;
    Ok(app)
}

/// Has the launcher give the links back to the handler it replaced, then
/// deletes it.
pub fn remove() -> Result<Option<PathBuf>, Box<dyn Error>> {
    let app = path()?;
    if !app.exists() {
        return Ok(None);
    }

    let mut unregister = Command::new(app.join(EXECUTABLE));
    unregister.arg("unregister");
    // Gone, the app hands no link to Postal even when unregister failed.
    let unregistered = run(unregister);
    removed(&app)?;
    unregistered?;
    Ok(Some(app))
}
