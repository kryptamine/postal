use std::{
    env,
    io::{self, Write},
    process::{Command, ExitCode, Stdio},
    thread,
};

use serde::{Deserialize, Serialize};

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

/// What the process writes to stdout.
#[derive(Serialize, Deserialize)]
enum Reply {
    People(Vec<Person>),
    Denied,
    Failed(String),
}

impl Source for ContactsProcess {
    fn people(&self) -> Result<Vec<Person>, Unavailable> {
        let failed = |error: &dyn std::fmt::Display| Unavailable::Failed(error.to_string());
        let exe = env::current_exe().map_err(|error| failed(&error))?;
        let output = Command::new(exe)
            .arg(ARG)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|error| failed(&error))?;
        let status = output.status;
        if !status.success() {
            return Err(failed(&format!("the Contacts process failed: {status}")));
        }

        match serde_json::from_slice(&output.stdout).map_err(|error| failed(&error))? {
            Reply::People(people) => Ok(people),
            Reply::Denied => Err(Unavailable::Denied),
            Reply::Failed(reason) => Err(Unavailable::Failed(reason)),
        }
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

    let reply = match read.join() {
        Ok(Ok(people)) => Reply::People(people),
        Ok(Err(Unavailable::Denied)) => Reply::Denied,
        Ok(Err(Unavailable::Failed(reason))) => Reply::Failed(reason),
        Err(_panic) => Reply::Failed("reading Contacts panicked".to_owned()),
    };

    let written =
        serde_json::to_vec(&reply).is_ok_and(|bytes| io::stdout().write_all(&bytes).is_ok());
    Some(if written {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
