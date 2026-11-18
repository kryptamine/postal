use ratatui::layout::Rect;

use crate::message_list;

/// From this width the Preview sits right of the Message List, else below.
const SIDE_BY_SIDE: u16 = 84;
/// The Message List's share of the width beside the Preview, in percent.
const LIST_WIDTH: u32 = 35;
/// The Message List's share of the height above the Preview, in percent.
const LIST_HEIGHT: u32 = 45;

/// Where the main screen's parts go: the panes, and the hint line on the
/// bottom row. Too narrow for a list, there is no Preview, so the list says
/// so on its own.
pub struct Panes {
    pub list: Rect,
    pub preview: Option<Rect>,
    pub hint: Rect,
    /// Everything above the hint line.
    pub body: Rect,
}

pub fn split(area: Rect) -> Panes {
    let body = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };

    let hint = Rect {
        y: body.bottom(),
        height: area.height - body.height,
        ..area
    };

    let (list, preview) = panes(body);
    Panes {
        list,
        preview,
        hint,
        body,
    }
}

fn panes(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < message_list::MIN_WIDTH {
        return (area, None);
    }

    if area.width >= SIDE_BY_SIDE {
        let width = share(area.width, LIST_WIDTH);
        let preview = Rect {
            x: area.x + width,
            width: area.width - width,
            ..area
        };

        (Rect { width, ..area }, Some(preview))
    } else {
        let height = share(area.height, LIST_HEIGHT);
        let preview = Rect {
            y: area.y + height,
            height: area.height - height,
            ..area
        };

        (Rect { height, ..area }, Some(preview))
    }
}

fn share(length: u16, percent: u32) -> u16 {
    u16::try_from(u32::from(length) * percent / 100).unwrap_or(length)
}
