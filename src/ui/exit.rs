use crate::domain::per_account::AccountId;

/// How the TUI ended.
pub enum Exit {
    Quit,
    /// `L` asked for the login flow of this Account, run with the terminal
    /// handed back.
    SignIn(AccountId),
    /// Settings asked for the Onboarding from the Provider list.
    AddAccount,
    /// Settings asked to unlink the Account of this address.
    Unlink(String),
    /// Settings changed what the Accounts read at start.
    Reload,
}
