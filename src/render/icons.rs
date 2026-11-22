use crate::{account::config::IconChoice, domain::folder::Role, domain::shown::Shown};

/// Nerd Font glyphs (v3 code points), or the plain look that any font draws.
#[derive(Clone, Copy)]
pub enum Icons {
    Nerd,
    Plain,
}

impl Icons {
    /// A terminal cannot report its font, so `auto` trusts only the terminals
    /// that bundle the Nerd Font symbols whatever font is set.
    pub fn choose(choice: IconChoice) -> Self {
        let bundled = || {
            let var = |name| std::env::var(name).unwrap_or_default();
            matches!(var("TERM_PROGRAM").as_str(), "ghostty" | "WezTerm")
                || matches!(
                    var("TERM").as_str(),
                    "xterm-kitty" | "xterm-ghostty" | "wezterm"
                )
                || std::env::var_os("KITTY_WINDOW_ID").is_some()
        };

        match choice {
            IconChoice::Nerd => Self::Nerd,
            IconChoice::Auto if bundled() => Self::Nerd,
            IconChoice::Auto | IconChoice::None => Self::Plain,
        }
    }

    /// The Unread marker on a card.
    pub const fn unread(self) -> &'static str {
        match self {
            Self::Nerd => "\u{f01ee}",
            Self::Plain => "●",
        }
    }

    /// On a read card whose Thread has a Reply sent; `None` in the plain look.
    pub const fn answered(self) -> Option<&'static str> {
        match self {
            Self::Nerd => Some("\u{f045a}"),
            Self::Plain => None,
        }
    }

    /// Marks a Thread or a Message with an attachment.
    pub const fn attachment(self) -> &'static str {
        match self {
            Self::Nerd => "\u{f03e2}",
            Self::Plain => "+",
        }
    }

    /// Before a Folder's name; `None` in the plain look.
    pub const fn folder(self, shown: &Shown) -> Option<&'static str> {
        if matches!(self, Self::Plain) {
            return None;
        }

        let role = match shown {
            Shown::AllInboxes { .. } => return Some("\u{f08b0}"),
            Shown::LocalDrafts => return Some("\u{f0322}"),
            Shown::Folder(folder) => folder.role,
        };

        Some(match role {
            Some(Role::Inbox) => "\u{f0687}",
            Some(Role::Starred) => "\u{f04ce}",
            Some(Role::Sent) => "\u{f048a}",
            Some(Role::Drafts) => DRAFT,
            Some(Role::All) => "\u{f0ee7}",
            Some(Role::Spam) => "\u{f0029}",
            Some(Role::Trash) => "\u{f01b4}",
            None => "\u{f024b}",
        })
    }

    /// The kind of a row in Local Drafts: an Outbox Message or a Draft.
    pub const fn local(self, queued: bool) -> &'static str {
        match (self, queued) {
            (Self::Nerd, true) => "\u{f048a}",
            (Self::Nerd, false) => DRAFT,
            (Self::Plain, true) => "Outbox",
            (Self::Plain, false) => "Draft",
        }
    }
}

const DRAFT: &str = "\u{f0dc8}";
