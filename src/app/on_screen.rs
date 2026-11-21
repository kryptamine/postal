use std::error::Error;

use crate::ui::terminal_slot::TerminalSlot;

/// Runs `work`, which takes the terminal once it draws, and hands the
/// terminal back as Postal found it.
pub fn on_screen<T>(
    work: impl AsyncFnOnce(&mut TerminalSlot) -> Result<T, Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async { work(&mut TerminalSlot::new()).await })
}
