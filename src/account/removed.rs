use std::{io, path::Path};

/// Removes `path`, a file or a directory; `false` when there was none.
pub fn removed(path: &Path) -> io::Result<bool> {
    let gone = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };

    match gone {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
