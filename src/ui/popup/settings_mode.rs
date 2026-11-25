/// What Settings is in the middle of: the keys mean something else in each.
pub enum Mode {
    Browsing,
    /// The text of `Field` being typed.
    Typing(Field, String),
    /// The Account at this address, which `d` asked to unlink, until `y`
    /// or any other key.
    Unlinking(String),
}

/// A setting typed rather than chosen.
#[derive(Clone, Copy)]
pub enum Field {
    Name,
    /// `send-delay`, in whole seconds.
    SendDelay,
    /// The program a mailto: link runs in place of a terminal.
    TerminalCommand,
}
