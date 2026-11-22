use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use lettre::message::{
    Body, SinglePart,
    header::{ContentDisposition, ContentTransferEncoding, ContentType},
};

/// Gmail refuses a Message whose files add up to more, in bytes.
pub const LIMIT: u64 = 25 * 1024 * 1024;

/// One `Attach:` line of a Draft, the file as written. `~` is the home
/// directory, and a relative path is under it.
pub struct Attach {
    pub written: String,
}

impl Attach {
    /// Fails when the file does not open or is not a regular file.
    pub fn size(&self) -> Result<u64, String> {
        let file = self.open()?;
        let metadata = file.metadata().map_err(|error| self.failed(&error))?;
        if !metadata.is_file() {
            return Err(format!("`Attach: {}`: not a file", self.written));
        }

        Ok(metadata.len())
    }

    /// The file as a part of the Message: base64, its type guessed from
    /// its extension, its name RFC 2231-encoded when not ASCII.
    pub fn part(&self) -> Result<SinglePart, String> {
        let path = self.path()?;
        let mut bytes = Vec::new();
        self.open()?
            .read_to_end(&mut bytes)
            .map_err(|error| self.failed(&error))?;
        let name = path.file_name().map_or_else(
            || self.written.clone(),
            |name| name.to_string_lossy().into_owned(),
        );
        let kind = mime_guess::from_path(&path).first_or_octet_stream();
        let kind = ContentType::parse(kind.essence_str()).map_err(|error| error.to_string())?;
        let body = Body::new_with_encoding(bytes, ContentTransferEncoding::Base64)
            .map_err(|_unencodable| self.failed(&"cannot be encoded"))?;
        Ok(SinglePart::builder()
            .header(kind)
            .header(ContentDisposition::attachment(&name))
            .body(body))
    }

    /// Resolved as the file is read, so a Draft parses without `$HOME`.
    fn path(&self) -> Result<PathBuf, String> {
        let under = self.written.strip_prefix("~/").unwrap_or(&self.written);
        if Path::new(under).is_absolute() {
            return Ok(PathBuf::from(under));
        }

        let home = std::env::var_os("HOME").filter(|home| !home.is_empty());
        let home = home.ok_or_else(|| self.failed(&"no home directory"))?;
        Ok(Path::new(&home).join(under))
    }

    fn open(&self) -> Result<File, String> {
        File::open(self.path()?).map_err(|error| self.failed(&error))
    }

    fn failed(&self, error: &dyn std::fmt::Display) -> String {
        format!("`Attach: {}`: {error}", self.written)
    }
}
