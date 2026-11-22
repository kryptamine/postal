use std::{error::Error, path::Path};

use crate::{
    account::atomic_file, account::config_file::ConfigFile, account::partial_config::PartialConfig,
    account::signature::Signature, compose::editor, compose::unique_id::unique_id,
};

/// Only the user may read their own writing.
const MODE: u32 = 0o600;

/// The editor on the signature of the Account `email`, as `config.toml` in
/// `dir` has it: on its file when one is named, else on its text, written
/// back to `config.toml` when it changed, which is `true`.
pub fn edit(dir: &Path, email: &str) -> Result<bool, Box<dyn Error>> {
    let config = dir.join("config.toml");
    let before = match PartialConfig::load(&config)?.signature_of(email) {
        Some(Signature::File(file)) => {
            editor::edit(&file)?;
            return Ok(false);
        }
        Some(written) => written.text()?,
        None => None,
    };

    let name = format!("postal-signature-{}.txt", unique_id());
    let scratch = std::env::temp_dir().join(name);
    let shown = before
        .as_ref()
        .map_or_else(String::new, |text| format!("{text}\n"));
    atomic_file::write(&scratch, shown.as_bytes(), MODE)?;
    let edited = editor::edit(&scratch).and_then(|()| std::fs::read_to_string(&scratch));
    if let Err(error) = std::fs::remove_file(&scratch) {
        tracing::warn!("removing {} failed: {error}", scratch.display());
    }

    let after = Signature::Text(edited?).text()?;
    if after == before {
        return Ok(false);
    }

    let mut config = ConfigFile::load(&config)?;
    config.set_signature(email, after.as_deref());
    config.save()?;
    Ok(true)
}
