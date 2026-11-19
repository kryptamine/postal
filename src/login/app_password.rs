use std::path::Path;

use super::{Error, save, verify, writes};
use crate::{
    browser, config::Auth, credential::Credential, partial_config::PartialConfig, prompt,
    sign_in::SignIn,
};

const APP_PASSWORDS: &str = "https://myaccount.google.com/apppasswords";

/// The Onboarding with an app password: asks until the server accepts one,
/// and writes nothing before it does. The email prompt starts from `email`.
#[expect(
    clippy::print_stdout,
    reason = "sign-in runs before the TUI and talks on stdout"
)]
pub fn app_password(dir: &Path, email: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let partial = PartialConfig::load(&dir.join("config.toml"))?;
    let email = ask("Email:", email.unwrap_or_default(), true)?;
    let writes = writes(&partial, &email, Auth::AppPassword)?;
    println!(
        "Google gives app passwords only to an Account with 2-Step Verification.\n\
         Create one for Postal at {APP_PASSWORDS}"
    );
    if let Err(error) = browser::open(APP_PASSWORDS) {
        tracing::warn!("cannot open the browser: {error}");
    }

    let config = partial.account(email, Auth::AppPassword);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let sign_in = loop {
        let pasted = ask("App password:", "", false)?;
        let sign_in = SignIn::Password(Credential::new(pasted.split_whitespace().collect()));
        match runtime.block_on(verify(&config, &sign_in)) {
            Ok(()) => break sign_in,
            Err(Error::Imap(error)) if error.bad_credentials() => {
                println!("The server refused this app password; paste it again.");
            }
            Err(error) => return Err(error.into()),
        }
    };

    save(dir, &config, &sign_in, writes)
}

/// Prompts until the answer holds more than whitespace.
fn ask(label: &str, initial: &str, echo: bool) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        let line = prompt::read_line(label, initial, echo)?.ok_or(Error::Cancelled)?;
        if !line.trim().is_empty() {
            return Ok(line.trim().to_owned());
        }
    }
}
