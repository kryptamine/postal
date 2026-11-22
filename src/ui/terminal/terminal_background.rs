use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use crate::render::rgb::Rgb;

/// The longest a terminal may take to answer, paid only by one that
/// answers neither query.
const WAIT: Duration = Duration::from_millis(100);

/// The terminal's background colour, asked with OSC 11 once raw mode is on.
/// DA1 goes after it: every terminal answers DA1, and answers in order, so
/// one that ignores OSC 11 costs no wait.
pub fn query() -> Option<Rgb> {
    let mut out = io::stdout();
    out.write_all(b"\x1b]11;?\x1b\\\x1b[c").ok()?;
    out.flush().ok()?;
    parse(&replies()?)
}

/// What the terminal sent, up to the end of its DA1 answer.
fn replies() -> Option<Vec<u8>> {
    let deadline = Instant::now() + WAIT;
    let mut replies = Vec::new();
    while !answered_da1(&replies) {
        let left = deadline.checked_duration_since(Instant::now())?;
        let mut ready = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };

        let timeout = i32::try_from(left.as_millis()).unwrap_or(i32::MAX);
        // SAFETY: `ready` is one valid pollfd, and the count says one.
        if unsafe { libc::poll(&raw mut ready, 1, timeout) } <= 0 {
            return None;
        }

        // Read straight from the fd: `io::stdin` buffers, and would keep
        // bytes past the answer from crossterm.
        let mut chunk = [0u8; 64];
        // SAFETY: `chunk` is writable for its whole length.
        let read =
            unsafe { libc::read(libc::STDIN_FILENO, chunk.as_mut_ptr().cast(), chunk.len()) };
        let read = usize::try_from(read).ok().filter(|&n| n > 0)?;
        replies.extend_from_slice(&chunk[..read]);
    }

    Some(replies)
}

/// DA1's answer is `ESC [ ? ... c`.
fn answered_da1(replies: &[u8]) -> bool {
    replies
        .windows(3)
        .position(|w| w == b"\x1b[?")
        .is_some_and(|start| replies[start..].contains(&b'c'))
}

/// `ESC ] 11 ; rgb:RRRR/GGGG/BBBB`, each channel one to four hex digits.
fn parse(replies: &[u8]) -> Option<Rgb> {
    const PREFIX: &[u8] = b"\x1b]11;rgb:";
    let start = replies.windows(PREFIX.len()).position(|w| w == PREFIX)? + PREFIX.len();
    let rest = &replies[start..];
    let end = rest.iter().position(|&b| b == 0x07 || b == 0x1b)?;
    let mut channels = std::str::from_utf8(&rest[..end])
        .ok()?
        .split('/')
        .map(channel);
    let rgb = Rgb {
        r: channels.next()??,
        g: channels.next()??,
        b: channels.next()??,
    };

    channels.next().is_none().then_some(rgb)
}

fn channel(hex: &str) -> Option<u8> {
    let digits = u32::try_from(hex.len())
        .ok()
        .filter(|n| (1..=4).contains(n))?;
    let value = u32::from_str_radix(hex, 16).ok()?;
    let max = 16u32.pow(digits) - 1;
    u8::try_from(value * 255 / max).ok()
}
