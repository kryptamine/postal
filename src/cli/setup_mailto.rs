use std::error::Error;

use crate::{account::mailto_handler, cli::command_output};

/// `postal setup-mailto`, or with `--remove` its undoing.
pub fn run(remove: bool) -> Result<(), Box<dyn Error>> {
    let said = if remove {
        mailto_handler::remove()?.map_or_else(
            || "Postal does not handle mailto: links\n".to_owned(),
            |path| {
                let path = path.display();
                format!("removed {path}: mailto: links go to the handler before Postal\n")
            },
        )
    } else {
        let path = mailto_handler::set_up()?;
        format!("mailto: links open in Postal through {}\n", path.display())
    };

    command_output::print(&said)?;
    Ok(())
}
