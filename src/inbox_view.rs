use chrono::{DateTime, FixedOffset, Local, Offset, Utc};

use crate::{
    inbox::Inbox,
    keymap::{By, Direction, Move, Scroll},
    message::Message,
    search::Search,
    view::View,
};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// `None` until the Cache or the server has rows to show. Their dates
    /// are local, each at the offset in force when it was received.
    pub messages: Option<Vec<Message>>,
    /// Narrows the rows shown; `selected` and `offset` index those.
    pub search: Option<Search>,
    /// The server's count over the whole Inbox.
    pub unread: usize,
    pub selected: usize,
    /// Index of the first visible card; kept by the widget so the selection stays on screen.
    pub offset: usize,
    /// The Preview's first row shown; back to 0 whenever the selection
    /// moves to another Message, and kept in range by the Preview widget.
    pub preview_scroll: usize,
    /// How the Preview shows HTML mail, for every Message.
    pub html: View,
    /// The local offset as of the last `show`: chrono's `Local` rereads the
    /// time zone every second, and that allocates inside a frame.
    zone: FixedOffset,
}

impl InboxView {
    pub fn loading() -> Self {
        Self {
            messages: None,
            search: None,
            unread: 0,
            selected: 0,
            offset: 0,
            preview_scroll: 0,
            html: View::default(),
            zone: Utc.fix(),
        }
    }

    /// Shows rows already ordered newest first. The selection stays on the
    /// same Message, or on the same row when that Message is gone.
    pub fn show(&mut self, mut inbox: Inbox) {
        let selected = self.selected_uid();
        for message in &mut inbox.messages {
            message.received = message.received.with_timezone(&Local).fixed_offset();
        }

        if let Some(search) = &mut self.search {
            search.reload(&inbox.messages);
        }

        self.messages = Some(inbox.messages);
        let last = self.rows().saturating_sub(1);
        self.selected = selected
            .and_then(|uid| self.position(uid))
            .unwrap_or_else(|| self.selected.min(last));
        if self.selected_uid() != selected {
            self.preview_scroll = 0;
        }

        self.unread = inbox.unread;
        self.zone = Local::now().offset().fix();
    }

    /// Starts a Search, or resumes the one under way; false before any
    /// rows. Either way, Esc while typing comes back to the selection now.
    pub fn start_search(&mut self) -> bool {
        let before = self.selected_uid();
        let Some(messages) = self.messages.as_deref() else {
            return false;
        };

        self.search
            .get_or_insert_with(|| Search::new(messages))
            .before = before;
        true
    }

    /// Edits the Query and selects the best match.
    pub fn edit_query(&mut self, edit: impl FnOnce(&mut Search)) {
        let selected = self.selected_uid();
        let Some(search) = &mut self.search else {
            return;
        };

        edit(search);
        self.selected = 0;
        if self.selected_uid() != selected {
            self.preview_scroll = 0;
        }
    }

    /// Back to the full list, on the Message selected before the last `/`
    /// when `restore`, else on the one selected now.
    pub fn end_search(&mut self, restore: bool) {
        let selected = self.selected_uid();
        let Some(search) = self.search.take() else {
            return;
        };

        let keep = if restore { search.before } else { selected };
        self.selected = keep.and_then(|uid| self.position(uid)).unwrap_or(0);
        if self.selected_uid() != selected {
            self.preview_scroll = 0;
        }
    }

    pub fn now(&self) -> DateTime<FixedOffset> {
        Utc::now().with_timezone(&self.zone)
    }

    pub fn apply(&mut self, movement: Move) {
        let last = self.rows().saturating_sub(1);
        let selected = match movement {
            Move::Down => (self.selected + 1).min(last),
            Move::Up => self.selected.saturating_sub(1),
            Move::First => 0,
            Move::Last => last,
        };

        if selected != self.selected {
            self.selected = selected;
            self.preview_scroll = 0;
        }
    }

    /// Past the last page is fine: the Preview widget clamps it.
    pub const fn scroll(&mut self, scroll: Scroll, half_page: usize) {
        let rows = match scroll.by {
            By::Line => 1,
            By::HalfPage => half_page,
            By::End => usize::MAX,
        };

        self.preview_scroll = match scroll.direction {
            Direction::Down => self.preview_scroll.saturating_add(rows),
            Direction::Up => self.preview_scroll.saturating_sub(rows),
        };
    }

    pub fn selected_uid(&self) -> Option<u32> {
        self.selected_message().map(|m| m.uid)
    }

    pub fn selected_message(&self) -> Option<&Message> {
        self.row(self.selected)
    }

    /// How many rows are shown: all, or those a Search narrowed them to.
    pub fn rows(&self) -> usize {
        let all = self.messages.as_ref().map_or(0, Vec::len);
        self.search.as_ref().map_or(all, Search::rows)
    }

    pub fn row(&self, row: usize) -> Option<&Message> {
        let index = match &self.search {
            Some(search) => search.row(row)?,
            None => row,
        };

        self.messages.as_ref()?.get(index)
    }

    /// The row that shows the Message `uid`.
    fn position(&self, uid: u32) -> Option<usize> {
        let index = self.messages.as_ref()?.iter().position(|m| m.uid == uid)?;
        self.search
            .as_ref()
            .map_or(Some(index), |search| search.position(index))
    }
}
