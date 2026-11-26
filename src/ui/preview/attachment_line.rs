use std::{fmt::Write, ops::Range};

use ratatui::{buffer::Buffer, layout::Rect};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::attachment::{Attachment, Attachments},
    render::file_size,
    render::stack_str::StackStr,
    ui::frame::theme::Theme,
    ui::preview::hint_keys::HintKeys,
    ui::widgets::truncate,
};

const SEPARATOR: &str = " · ";

/// Draws `attachments` in `area`, `name size` each, as many as fit with a
/// count of the rest; in hint mode, each after its label in `labels`.
pub fn draw(
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
    attachments: &Attachments,
    labels: Option<&HintKeys>,
) {
    let label = |index| labels.and_then(|labels| labels.label(index));
    let total = attachments.iter().count();
    let fitting = fitting(attachments, usize::from(area.width), labels);

    let mut x = area.x;
    let mut put = |text: &str, style| {
        let left = area.right().saturating_sub(x);
        let cell = Rect {
            x,
            width: left,
            ..area
        };

        if !cell.is_empty() {
            truncate::draw(buf, cell, text, style);
        }

        x = x.saturating_add(u16::try_from(text.width()).unwrap_or(u16::MAX));
    };

    if fitting == 0 {
        // Too narrow for one with the count: the name alone, or the count.
        if let Some(only) = attachments.iter().next().filter(|_| total == 1) {
            if let Some(label) = label(0) {
                put(label.as_str(), theme.label);
                put(" ", theme.muted);
            }

            put(only.name, theme.sender);
        } else {
            let mut count = StackStr::<64>::new();
            // The keys are capped below the room left, so the writes cannot fail.
            write!(count, "{total} attachments").ok();
            write_keys(&mut count, 0..total, labels);
            put(count.as_str(), theme.muted);
        }

        return;
    }

    for (index, attachment) in attachments.iter().take(fitting).enumerate() {
        if index > 0 {
            put(SEPARATOR, theme.muted);
        }

        if let Some(label) = label(index) {
            put(label.as_str(), theme.label);
            put(" ", theme.muted);
        }

        put(attachment.name, theme.sender);
        put(" ", theme.muted);
        put(file_size::shown(attachment.size).as_str(), theme.muted);
    }

    if fitting < total {
        put(SEPARATOR, theme.muted);
        put(rest(fitting..total, labels).as_str(), theme.muted);
    }
}

/// Every one of `attachments`, `name size`, ` · ` apart.
pub fn write(out: &mut String, attachments: &Attachments) {
    for (index, attachment) in attachments.iter().enumerate() {
        let separator = if index == 0 { "" } else { SEPARATOR };
        let size = file_size::shown(attachment.size);
        // Writing to a String cannot fail.
        write!(out, "{separator}{} {}", attachment.name, size.as_str()).ok();
    }
}

/// How many of `attachments` fit `room` columns with the count of the rest.
fn fitting(attachments: &Attachments, room: usize, labels: Option<&HintKeys>) -> usize {
    let total = attachments.iter().count();
    let mut fitting = 0;
    let mut used = 0;
    for (index, attachment) in attachments.iter().enumerate() {
        let label = labels.and_then(|labels| labels.label(index));
        let labelled = label.map_or(0, |label| label.as_str().width() + 1);
        used += if index == 0 { 0 } else { SEPARATOR.width() } + labelled + width(&attachment);
        let shown = index + 1;
        let rest = if shown == total {
            0
        } else {
            SEPARATOR.width() + rest(shown..total, labels).as_str().width()
        };

        if used + rest > room {
            break;
        }

        fitting = shown;
    }

    fitting
}

fn width(attachment: &Attachment<'_>) -> usize {
    attachment.name.width() + 1 + file_size::shown(attachment.size).as_str().width()
}

fn rest(hidden: Range<usize>, labels: Option<&HintKeys>) -> StackStr<64> {
    let mut rest = StackStr::new();
    // The keys are capped below the room left, so the writes cannot fail.
    write!(rest, "+{} more", hidden.len()).ok();
    write_keys(&mut rest, hidden, labels);
    rest
}

/// The hint keys of the `hidden` attachments, as ` (s, d)`, in hint mode;
/// those past `KEYS` bytes become `…`.
fn write_keys(into: &mut StackStr<64>, hidden: Range<usize>, labels: Option<&HintKeys>) {
    const KEYS: usize = 32;
    const MORE: &str = ", …";
    let Some(labels) = labels else {
        return;
    };

    let mut keys = StackStr::<KEYS>::new();
    for key in hidden.filter_map(|index| labels.label(index)) {
        let (written, key) = (keys.as_str().len(), key.as_str());
        let gap = if written == 0 { "" } else { ", " };
        if written + gap.len() + key.len() + MORE.len() > KEYS {
            keys.write_str(MORE).ok();
            break;
        }

        keys.write_str(gap).ok();
        keys.write_str(key).ok();
    }

    if !keys.as_str().is_empty() {
        write!(into, " ({})", keys.as_str()).ok();
    }
}
