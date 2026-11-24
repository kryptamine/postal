/// What Settings is in the middle of: the keys mean something else in each.
pub enum Mode {
    Browsing,
    /// The Name being typed.
    Naming(String),
    /// The Account at this address, which `d` asked to unlink, until `y`
    /// or any other key.
    Unlinking(String),
}
