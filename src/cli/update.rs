use std::{
    env,
    error::Error,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::cli::{
    command_output,
    release::{self, ARCHIVE},
    version::Version,
};

/// Replaces the running binary with the latest release, when that is newer.
pub fn run() -> Result<(), Box<dyn Error>> {
    let exe = env::current_exe()?.canonicalize()?;
    let dir = exe.parent().ok_or("postal runs from no directory")?;
    if cargo_bin().is_some_and(|bin| bin == dir) {
        return Err(format!(
            "{} is cargo's; update it with `git pull && make install`",
            exe.display()
        )
        .into());
    }

    let running = Version::running()?;
    let latest = release::latest()?;
    if latest <= running {
        command_output::print(&format!("postal {running} is the latest release\n"))?;
        return Ok(());
    }

    let archive = release::archive(latest)?;
    let scratch = Scratch::new(dir)?;
    let binary = scratch.unpack(&archive)?;
    answers(&binary, latest)?;
    // A rename replaces the file a running postal still has open, rather than writing into it.
    fs::rename(&binary, &exe)?;
    command_output::print(&format!("postal {running} updated to {latest}\n"))?;
    Ok(())
}

/// Where `cargo install`, and so `make install`, puts a binary.
fn cargo_bin() -> Option<PathBuf> {
    let home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")))?;
    home.join("bin").canonicalize().ok()
}

/// Checks that the new binary runs on this machine and is the release
/// expected, before it replaces the one that works.
fn answers(binary: &Path, version: Version) -> Result<(), String> {
    let expected = format!("postal {version}\n");
    let why = match Command::new(binary).arg("--version").output() {
        Ok(output) if output.status.success() && output.stdout == expected.as_bytes() => {
            return Ok(());
        }
        Ok(output) => format!(
            "{}, {}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Err(error) => error.to_string(),
    };

    Err(format!("the release {version} does not run here: {why}"))
}

/// A directory beside the binary, so the final rename stays on one
/// filesystem; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(dir: &Path) -> Result<Self, String> {
        let path = dir.join(format!(".postal-update-{}", std::process::id()));
        fs::create_dir(&path)
            .map_err(|error| format!("cannot write to {}: {error}", dir.display()))?;
        Ok(Self(path))
    }

    /// The binary inside `archive`, executable.
    fn unpack(&self, archive: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
        let mut tar = Command::new("tar")
            .args(["-xzf", "-", "-C"])
            .arg(&self.0)
            .stdin(Stdio::piped())
            .spawn()?;
        // Dropping stdin closes it, which ends the archive for tar.
        let fed = tar.stdin.take().map(|mut stdin| stdin.write_all(archive));
        let status = tar.wait()?;
        if !status.success() || !matches!(fed, Some(Ok(()))) {
            return Err(format!("tar could not unpack {ARCHIVE}.tar.gz: {status}").into());
        }

        let binary = self.0.join(ARCHIVE).join("postal");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
        Ok(binary)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}
