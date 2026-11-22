use std::fmt::Write;

/// A file a Message carries beside its text.
pub struct Attachment<'a> {
    /// Its file name, else what it is: an attached Message's subject or the
    /// MIME type.
    pub name: &'a str,
    /// In octets once decoded, as far as the size sent tells.
    pub size: u64,
}

/// A Message's attachments in its order, one `size\tname` line each.
#[derive(Clone, Default)]
pub struct Attachments {
    stored: String,
}

impl Attachments {
    /// A control character in `name` becomes a space, so a line holds one.
    pub fn push(&mut self, name: &str, size: u64) {
        // Writing to a String cannot fail.
        write!(self.stored, "{size}\t").ok();
        self.stored
            .extend(name.chars().map(|c| if c.is_control() { ' ' } else { c }));
        self.stored.push('\n');
    }

    pub const fn is_empty(&self) -> bool {
        self.stored.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = Attachment<'_>> + Clone {
        self.stored.lines().filter_map(|line| {
            let (size, name) = line.split_once('\t')?;
            Some(Attachment {
                name,
                size: size.parse().ok()?,
            })
        })
    }

    pub fn stored(&self) -> &str {
        &self.stored
    }

    /// The lines, to be overwritten with their buffer reused.
    pub const fn stored_mut(&mut self) -> &mut String {
        &mut self.stored
    }
}
