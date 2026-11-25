use std::{io, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::time::Instant;

use crate::{
    mail::handover,
    mail::sync_lock::{self, Holder, Kind, SyncLock},
    ui::frame::theme::Theme,
    ui::takeover::move_popup::MovePopup,
    ui::terminal::tty::Tty,
};

/// How long a TUI asked to move has to quit before force quit is offered.
const ANSWER: Duration = Duration::from_secs(5);
/// In place of `ANSWER` while a Send is under way there: the 15 s the TUI
/// waits on it, then `ANSWER`.
const SEND: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(100);

/// The lock, from whatever holds it: a TUI in another terminal moves here
/// if the user says so, a folder sync is waited for. `None` once the user
/// declines or quits.
pub async fn move_here(mut tty: Tty<'_>, theme: &Theme) -> io::Result<Option<SyncLock>> {
    loop {
        if let Some(lock) = SyncLock::try_take(Kind::Tui)? {
            return Ok(Some(lock));
        }

        match sync_lock::holder()? {
            Some(holder) if holder.kind == Kind::Tui => {
                return moved(&mut tty, theme, &holder).await;
            }

            Some(_) => {
                if !waited(&mut tty, theme, "Waiting for a folder sync to finish…").await? {
                    return Ok(None);
                }
            }
            // A holder that names none: 0.5.0 or older, or one just gone.
            None => {
                if !waited(&mut tty, theme, "Waiting for another Postal to quit…").await? {
                    return Ok(None);
                }
            }
        }
    }
}

/// Asks the TUI `holder` names to move here, then waits for its lock.
async fn moved(tty: &mut Tty<'_>, theme: &Theme, holder: &Holder) -> io::Result<Option<SyncLock>> {
    let other = holder.tty.as_deref().unwrap_or("another terminal");
    let open = format!("Postal is open in {other}.");
    let mut lines = vec![open.as_str()];
    if holder.busy.outside {
        lines.push("It has the editor or the pager open there, which stays open.");
    }

    lines.push("Move it here?");
    if !confirmed(tty, theme, &lines, "y move here · n quit").await? {
        return Ok(None);
    }

    // A holder gone since leaves the lock free.
    handover::ask(holder).ok();
    let asked = Instant::now();
    let mut deadline = Some(asked + ANSWER);
    loop {
        if let Some(lock) = SyncLock::try_take(Kind::Tui)? {
            handover::forget();
            return Ok(Some(lock));
        }

        let sending = sync_lock::holder()?.is_some_and(|now| now.busy.sending);
        if sending {
            deadline = deadline.map(|_| asked + SEND);
        }

        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let stuck = format!("Postal in {other} is not responding.");
            let lines = [stuck.as_str(), "Force quit it?"];
            if !confirmed(tty, theme, &lines, "y force quit · n quit").await? {
                return Ok(None);
            }

            handover::force(holder).ok();
            deadline = None;
        }

        let text = if sending {
            "Waiting for a Send to finish…"
        } else {
            "Moving Postal here…"
        };

        if !waited(tty, theme, text).await? {
            return Ok(None);
        }
    }
}

/// Shows `text` for a while; `false` once the user quits.
async fn waited(tty: &mut Tty<'_>, theme: &Theme, text: &str) -> io::Result<bool> {
    draw(tty, theme, &[text], "esc quit")?;
    let Ok(event) = tokio::time::timeout(POLL, tty.events.next()).await else {
        return Ok(true);
    };

    Ok(!matches!(answer(event)?, Some(false)))
}

/// Asks the question `lines` end with until the user answers it.
async fn confirmed(
    tty: &mut Tty<'_>,
    theme: &Theme,
    lines: &[&str],
    keys: &str,
) -> io::Result<bool> {
    loop {
        draw(tty, theme, lines, keys)?;
        if let Some(answer) = answer(tty.events.next().await)? {
            return Ok(answer);
        }
    }
}

/// Yes or no from a key; `None` for any other event. A terminal gone says no.
fn answer(event: Option<io::Result<Event>>) -> io::Result<Option<bool>> {
    let Some(event) = event else {
        return Ok(Some(false));
    };

    let Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        ..
    }) = event?
    else {
        return Ok(None);
    };

    Ok(match code {
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(false),
        KeyCode::Char('y' | 'Y') | KeyCode::Enter => Some(true),
        KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => Some(false),
        _ => None,
    })
}

fn draw(tty: &mut Tty<'_>, theme: &Theme, lines: &[&str], keys: &str) -> io::Result<()> {
    tty.terminal.draw(|frame| {
        let popup = MovePopup { theme, lines, keys };
        frame.render_widget(popup, frame.area());
    })?;
    Ok(())
}
