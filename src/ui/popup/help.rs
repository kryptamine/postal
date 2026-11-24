use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Position;

/// The `?` popup's filter and scroll position.
#[derive(Default)]
pub struct Help {
    /// What the rows are narrowed to; empty for all of them.
    pub query: String,
    /// The filter is being typed: keys go into `query`.
    pub filtering: bool,
    /// The first row of the body shown.
    pub scroll: usize,
    /// The furthest `scroll` goes, and the body's height, as last drawn.
    pub bottom: usize,
    pub height: usize,
    /// After the filter while it is typed, as last drawn.
    pub cursor: Option<Position>,
}

impl Help {
    /// Takes one pressed key; false when it closes the popup.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('d') => self.by(self.half()),
                KeyCode::Char('u') => self.by(-self.half()),
                KeyCode::Char('j') => self.by(1),
                KeyCode::Char('k') => self.by(-1),
                _ => {}
            }

            return true;
        }

        match key.code {
            KeyCode::Down => self.by(1),
            KeyCode::Up => self.by(-1),
            KeyCode::PageDown => self.by(self.half()),
            KeyCode::PageUp => self.by(-self.half()),
            _ if self.filtering => self.filter(key.code),
            KeyCode::Char('j') => self.by(1),
            KeyCode::Char('k') => self.by(-1),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = self.bottom,
            KeyCode::Char('/') => {
                self.filtering = true;
                self.scroll = 0;
            }
            KeyCode::Char('?' | 'q') | KeyCode::Esc | KeyCode::Enter => return false,
            _ => {}
        }

        true
    }

    /// A key while the filter is typed.
    fn filter(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => self.query.push(c),
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Enter => self.filtering = false,
            KeyCode::Esc => {
                self.query.clear();
                self.filtering = false;
            }
            _ => return,
        }

        self.scroll = 0;
    }

    fn half(&self) -> isize {
        isize::try_from(self.height / 2)
            .unwrap_or(isize::MAX)
            .max(1)
    }

    fn by(&mut self, rows: isize) {
        self.scroll = self.scroll.saturating_add_signed(rows).min(self.bottom);
    }
}
