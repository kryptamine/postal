use std::{
    mem,
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

use rusqlite::{Connection, OpenFlags};

use super::{Cache, Error};

/// An address an Account has mail from or has written to, with a name it
/// went by.
pub struct Correspondent {
    /// Lowercased.
    pub address: String,
    pub name: String,
    /// `name` lowercased, to match in any case.
    pub lower: String,
}

/// Every Account's Correspondents, those written to first, then by how much
/// mail names them. Read on a thread of its own: 0.8 s for 1.5 million
/// rows of `correspondence`.
pub struct Correspondents(State);

enum State {
    Reading(JoinHandle<Vec<Correspondent>>),
    Read(Vec<Correspondent>),
}

impl Default for Correspondents {
    fn default() -> Self {
        Self(State::Read(Vec::new()))
    }
}

impl Correspondents {
    /// Waits for the read while it is under way.
    pub fn get(&mut self) -> &[Correspondent] {
        if let State::Reading(_) = self.0
            && let State::Reading(reading) = mem::replace(&mut self.0, State::Read(Vec::new()))
        {
            self.0 = State::Read(reading.join().unwrap_or_default());
        }

        match &self.0 {
            State::Read(read) => read,
            State::Reading(_) => &[],
        }
    }
}

impl Cache {
    /// Starts reading every Account's Correspondents, on a connection of
    /// their own; none for a Cache in memory.
    pub fn correspondents(&self) -> Correspondents {
        let path = (self.connection().path())
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let Some(path) = path else {
            return Correspondents::default();
        };

        Correspondents(State::Reading(thread::spawn(move || {
            read(&path).unwrap_or_else(|error| {
                tracing::warn!("no Correspondents to suggest: {error}");
                Vec::new()
            })
        })))
    }
}

fn read(path: &Path) -> Result<Vec<Correspondent>, Error> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(
        "SELECT address, coalesce(max(name), '') FROM correspondence
         GROUP BY address ORDER BY max(written) DESC, count(*) DESC, address",
    )?;
    let read = statement
        .query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(Correspondent {
                address: row.get(0)?,
                lower: name.to_lowercase(),
                name,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(read)
}
