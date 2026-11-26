use std::{
    env,
    io::{self, Write},
    process::{Command, ExitCode, Stdio},
    thread,
};

use crate::account::{
    address_book::AddressBook,
    contacts::{Person, Source, Unavailable},
    contacts_fake,
};

/// The first argument that makes `postal` the Contacts process.
const ARG: &str = "__contacts";

/// Contacts read by `postal` started again as a process of its own: the
/// framework keeps its memory after a read, about 10 MB, and this gives it
/// back to the system when the process ends. See ADR 0035.
pub struct ContactsProcess;

impl Source for ContactsProcess {
    fn people(&self) -> Result<Vec<Person>, Unavailable> {
        let exe = env::current_exe().map_err(|error| Unavailable::Failed(error.to_string()))?;
        let output = Command::new(exe)
            .arg(ARG)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|error| Unavailable::Failed(error.to_string()))?;
        let status = output.status;
        if !status.success() {
            let reason = format!("the Contacts process failed: {status}");
            return Err(Unavailable::Failed(reason));
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|error| Unavailable::Failed(error.to_string()))?
    }
}

/// The Contacts process's whole run, when `postal` was started as one.
pub fn serve() -> Option<ExitCode> {
    if env::args_os().nth(1)? != ARG {
        return None;
    }

    // Off the main thread the store waits while macOS asks for access.
    let read = thread::spawn(|| {
        let fake = contacts_fake::from_env();
        fake.map_or_else(|| AddressBook.people(), |source| source.people())
    });

    let read = read
        .join()
        .unwrap_or_else(|_panic| Err(Unavailable::Failed("reading Contacts panicked".to_owned())));
    let written =
        serde_json::to_vec(&read).is_ok_and(|bytes| io::stdout().write_all(&bytes).is_ok());
    Some(if written {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
