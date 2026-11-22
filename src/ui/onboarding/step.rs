use super::server_form::ServerForm;
use crate::account::provider::Provider;

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
    /// The address being typed, for a sign-in by password at `provider`.
    Address {
        value: &'a str,
        provider: Provider,
    },
    /// The servers of a generic Provider being typed.
    Servers {
        heading: &'a str,
        form: &'a ServerForm,
    },
    /// The password being typed, `length` characters so far.
    Password {
        heading: &'a str,
        length: usize,
        provider: Provider,
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
