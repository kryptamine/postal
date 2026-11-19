/// The flags of a Message that the Message List shows.
#[derive(Clone, Copy)]
pub struct MessageFlags {
    pub unread: bool,
    /// `\Answered`: a Reply to it was sent, from Postal or elsewhere.
    pub answered: bool,
}

impl MessageFlags {
    /// From the flags as IMAP names them, in any case.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut flags = Self {
            unread: true,
            answered: false,
        };

        for name in names {
            flags.unread &= !name.eq_ignore_ascii_case("\\Seen");
            flags.answered |= name.eq_ignore_ascii_case("\\Answered");
        }

        flags
    }
}
