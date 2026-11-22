use std::{error::Error, io, path::Path};

use crate::{
    account::atomic_file, account::config_file::ConfigFile, account::partial_config::PartialConfig,
    account::signature::Signature, account::xdg, compose::editor, compose::unique_id::unique_id,
};

/// Only the user may read their own writing.
const MODE: u32 = 0o600;

/// The editor on the signature of the Account `email`, as `config.toml` in
/// `dir` gives it, in `$XDG_CACHE_HOME/postal/signatures`, where the plugin
/// knows it. `true` when `config.toml` changed: a signature file takes it.
pub fn edit(dir: &Path, email: &str) -> Result<bool, Box<dyn Error>> {
    let config = dir.join("config.toml");
    let written = PartialConfig::load(&config)?.signature_of(email);
    let before = match written.as_ref().map(Signature::text) {
        Some(Ok(text)) => text,
        // A file not yet written signs nothing so far.
        Some(Err(error)) if error.kind() == io::ErrorKind::NotFound => None,
        Some(Err(error)) => return Err(error.into()),
        None => None,
    };

    let scratch = xdg::cache_dir()?
        .join("signatures")
        .join(format!("{}.txt", unique_id()));
    atomic_file::write(&scratch, lined(before.as_deref()).as_bytes(), MODE)?;
    let edited = editor::edit(&scratch).and_then(|()| std::fs::read_to_string(&scratch));
    if let Err(error) = std::fs::remove_file(&scratch) {
        tracing::warn!("removing {} failed: {error}", scratch.display());
    }

    let after = Signature::Text(edited?).text()?;
    if after == before {
        return Ok(false);
    }

    if let Some(Signature::File(file)) = written {
        // Written in place: the file may be a link, and its mode is the user's.
        std::fs::write(&file, lined(after.as_deref()))?;
        return Ok(false);
    }

    let mut config = ConfigFile::load(&config)?;
    config.set_signature(email, after.as_deref());
    config.save()?;
    Ok(true)
}

/// `text` as a file holds it: ending in a newline, or empty.
fn lined(text: Option<&str>) -> String {
    text.map_or_else(String::new, |text| format!("{text}\n"))
}
