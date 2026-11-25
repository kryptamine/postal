use std::{error::Error, path::Path, process::Command};

/// Runs `command` to its end; its stderr is the error when it fails.
pub fn run(mut command: Command) -> Result<(), Box<dyn Error>> {
    let name = Path::new(command.get_program()).display().to_string();
    let output = command
        .output()
        .map_err(|error| format!("cannot run {name}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let said = String::from_utf8_lossy(&output.stderr);
    Err(format!("{name}: {}, {}", output.status, said.trim()).into())
}
