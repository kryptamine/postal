use std::error::Error;

use crate::{
    account::xdg, app::on_screen::on_screen, app::onboard::onboard, ui::onboarding::Start,
};

/// `postal login`: the Onboarding from the Provider list.
#[expect(
    clippy::print_stdout,
    reason = "the terminal is restored, so stdout reaches the user"
)]
pub fn run() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_dir()?;
    let email = on_screen(async |slot| onboard(slot, &dir, Start::Choose).await)?;
    println!("Signed in as {email}.");
    Ok(())
}
