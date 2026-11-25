use crate::{
    account::config::{
        ComposeFormat, ContactsChoice, IconChoice, NotifyChoice, SecretStore, TerminalApp,
    },
    account::mailto_handler::CHOOSES_TERMINAL,
    ui::popup::settings_section::Section,
};

/// A value a Settings tab lists, as `config.toml` takes it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Icons(IconChoice),
    Notify(NotifyChoice),
    Secrets(SecretStore),
    Terminal(TerminalApp),
    Contacts(ContactsChoice),
    Compose(ComposeFormat),
}

impl Value {
    /// The values `section` lists, in order; none for a tab of other rows.
    pub const fn of(section: Section) -> &'static [Self] {
        match section {
            Section::Icons => &[
                Self::Icons(IconChoice::Auto),
                Self::Icons(IconChoice::Nerd),
                Self::Icons(IconChoice::None),
            ],
            Section::Notify => &[
                Self::Notify(NotifyChoice::Unfocused),
                Self::Notify(NotifyChoice::Always),
                Self::Notify(NotifyChoice::Never),
            ],
            Section::Secrets => &[
                Self::Secrets(SecretStore::Keychain),
                Self::Secrets(SecretStore::File),
            ],
            Section::Mailto if CHOOSES_TERMINAL => &[
                Self::Terminal(TerminalApp::Terminal),
                Self::Terminal(TerminalApp::Ghostty),
                Self::Terminal(TerminalApp::Kitty),
                Self::Terminal(TerminalApp::Wezterm),
                Self::Terminal(TerminalApp::Iterm2),
            ],
            Section::Contacts => &[
                Self::Contacts(ContactsChoice::None),
                Self::Contacts(ContactsChoice::Macos),
            ],
            Section::Send => &[
                Self::Compose(ComposeFormat::Plain),
                Self::Compose(ComposeFormat::Markdown),
            ],
            Section::Accounts | Section::Name | Section::Mailto => &[],
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::Icons(icons) => icons.key(),
            Self::Notify(notify) => notify.key(),
            Self::Secrets(store) => store.key(),
            Self::Terminal(terminal) => terminal.key(),
            Self::Contacts(contacts) => contacts.key(),
            Self::Compose(format) => format.key(),
        }
    }

    /// What it does, as its row says beside it.
    pub const fn detail(self) -> &'static str {
        match self {
            Self::Icons(IconChoice::Auto) => "Nerd Font in Ghostty, WezTerm and kitty",
            Self::Icons(IconChoice::Nerd) => "always Nerd Font glyphs",
            Self::Icons(IconChoice::None) => "plain symbols",
            Self::Notify(NotifyChoice::Unfocused) => "while the terminal is in the background",
            Self::Notify(NotifyChoice::Always) => "even while Postal has focus",
            Self::Notify(NotifyChoice::Never) => "no notifications",
            Self::Secrets(SecretStore::Keychain) => "the OS keychain",
            Self::Secrets(SecretStore::File) => "files in secrets/ beside config.toml",
            Self::Terminal(TerminalApp::Terminal) => "a link opens Postal in Terminal.app",
            Self::Terminal(TerminalApp::Ghostty) => "in Ghostty",
            Self::Terminal(TerminalApp::Kitty) => "in kitty",
            Self::Terminal(TerminalApp::Wezterm) => "in WezTerm",
            Self::Terminal(TerminalApp::Iterm2) => "in iTerm2",
            Self::Contacts(ContactsChoice::None) => "Correspondents come from mail alone",
            Self::Compose(ComposeFormat::Plain) => "the text as written, one plain part",
            Self::Compose(ComposeFormat::Markdown) => {
                "the text, and its Markdown as HTML beside it"
            }
            Self::Contacts(ContactsChoice::Macos) => {
                "Contacts too: an agent through MCP sees them too"
            }
        }
    }
}
