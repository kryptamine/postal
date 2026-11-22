use std::{
    fs, io,
    path::{Path, PathBuf},
};

use mail_parser::MessageParser;

use crate::{
    account::xdg,
    domain::message::Message,
    render::body_text::{self, WIDTH},
    render::header_date,
    render::html_preview::Links,
};

/// A Message as a read-only file for the editor in
/// `$XDG_CACHE_HOME/postal/messages`, removed when dropped. The plugin knows
/// it by that directory.
pub struct MessageFile {
    path: PathBuf,
}

impl MessageFile {
    /// Writes `message` with its `raw` body: the header block, a blank line,
    /// and the body as plain text, its Quote included.
    pub fn write(message: &Message, raw: &[u8]) -> io::Result<Self> {
        let body = MessageParser::default()
            .parse(raw)
            .map(|parsed| body_text::plain(&parsed, WIDTH, Links::Footnotes))
            .unwrap_or_default();
        let text = format!(
            "From: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}\n",
            message.from,
            message.to,
            header_date::date(&message.received).as_str(),
            message.subject,
            body.trim_end()
        );

        let dir = xdg::cache_dir().map_err(io::Error::other)?.join("messages");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.eml", message.body_key));
        // One left by a run that crashed is read-only, so it cannot be
        // written over.
        match fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }

        let file = Self { path };
        fs::write(&file.path, text)?;
        let mut permissions = fs::metadata(&file.path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file.path, permissions)?;
        Ok(file)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MessageFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            tracing::warn!("removing {} failed: {error}", self.path.display());
        }
    }
}
