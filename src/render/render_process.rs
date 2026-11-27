use std::{
    io::{self, Read, Write},
    process::{Command, ExitCode, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::{render::cell_grid::CellGrid, render::litehtml};

/// The first argument that makes `postal` the layout process.
const ARG: &str = "__lay-out";
/// Far past what any mail takes, about 0.2 s for 400 KB; past it the
/// engine is taken to hang, and the process is killed.
const TIMEOUT: Duration = Duration::from_secs(5);

/// `html` laid out `cols` cells wide in a process of its own: litehtml's
/// memory is not handed back to the system after a document, and a crash
/// on hostile mail ends only that process. See ADR 0006.
pub fn lay_out(html: &str, cols: u16) -> Option<CellGrid> {
    let exe = std::env::current_exe().ok()?;
    let mut child = Command::new(exe)
        .arg(ARG)
        .arg(cols.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .inspect_err(|error| tracing::warn!("starting the layout process failed: {error}"))
        .ok()?;

    let mut stdout = child.stdout.take()?;
    let (done, output) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout.read_to_end(&mut bytes).map(|_| bytes);
        done.send(read).ok();
    });

    // The child reads all of it before it writes anything, and dropping
    // stdin closes it, which ends the HTML.
    let written = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(html.as_bytes()));
    let bytes = output.recv_timeout(TIMEOUT).ok().and_then(Result::ok);
    if bytes.is_none() {
        child.kill().ok();
    }

    let status = child.wait().ok()?;
    if !status.success() || !matches!(written, Some(Ok(()))) {
        tracing::warn!("the layout process failed: {status}");
        return None;
    }

    CellGrid::decode(&bytes?)
}

/// The layout process's whole run, when `postal` was started as one:
/// HTML on stdin, the grid on stdout: encoded, or as text after `text`.
pub fn serve() -> Option<ExitCode> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != ARG {
        return None;
    }

    // `text` goes before the cols, so a build without it fails on it.
    let first = args.next();
    let text = first.as_deref().is_some_and(|arg| arg == "text");
    let cols = if text { args.next() } else { first };
    let cols = cols.and_then(|cols| cols.to_str()?.parse().ok());
    let mut html = String::new();
    let grid = cols
        .filter(|_| io::stdin().read_to_string(&mut html).is_ok())
        .and_then(|cols| litehtml::lay_out(&html, cols));
    let bytes = grid.map(|grid| {
        if text {
            grid.text().into_bytes()
        } else {
            grid.encode()
        }
    });

    let written = bytes.is_some_and(|bytes| io::stdout().write_all(&bytes).is_ok());
    Some(if written {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
