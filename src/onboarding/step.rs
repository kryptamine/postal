/// What the Onboarding screen shows.
pub enum Step<'a> {
    /// The Provider list, with the one at `selected` chosen.
    Providers {
        selected: usize,
    },
    /// Waiting for the browser; `back` when `Esc` returns to the Providers.
    Browser {
        heading: &'a str,
        url: &'a str,
        copied: bool,
        back: bool,
    },
    Checking,
    /// Whether the new Account replaces the one the config named.
    Replace {
        question: &'a str,
    },
    Failed {
        error: &'a str,
    },
}
