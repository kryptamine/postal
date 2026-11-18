use std::io;

/// Stops Postal as the shell's job control expects, returning once it is
/// continued. The caller hands the terminal back first.
pub fn suspend() -> io::Result<()> {
    // SAFETY: `raise` with a valid signal number has no preconditions.
    if unsafe { libc::raise(libc::SIGTSTP) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
