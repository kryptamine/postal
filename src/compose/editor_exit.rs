/// How the editor on a Draft ended.
#[derive(Clone, Copy, Default)]
pub enum EditorExit {
    /// The file is kept as the editor left it; also when it failed to run.
    #[default]
    Saved,
    /// The editor asked for the Draft's Send, as `:PostalSend` does.
    Send,
}
