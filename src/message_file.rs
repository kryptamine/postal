use std::{
    fs, io,
    path::{Path, PathBuf},
};

use mail_parser::MessageParser;

use crate::{body_text, html_preview::Links, message::Message, message_header::DATE};

/// Columns an HTML body is laid out in: the line RFC 5322 recommends.
const WIDTH: usize = 78;

/// A Message as a read-only file for the editor, under `dir`, removed when
/// dropped. The plugin knows it by `messages/` in its path.
pub struct MessageFile {
    path: PathBuf,
}

impl MessageFile {
    /// Writes `message` with its `raw` body: the header block, a blank line,
    /// and the body as plain text, its Quote included.
    pub fn write(dir: &Path, message: &Message, raw: &[u8]) -> io::Result<Self> {
        let body = MessageParser::default()
            .parse(raw)
            .map(|parsed| body_text::plain(&parsed, WIDTH, Links::Footnotes))
            .unwrap_or_default();
        let text = format!(
            "From: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}\n",
            message.from,
            message.to,
            message.received.format(DATE),
            message.subject,
            body.trim_end()
        );

        fs::create_dir_all(dir)?;
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
