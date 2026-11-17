use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub unread_marker: Style,
    pub unread: Style,
    pub date: Style,
    pub selected: Style,
}

impl Theme {
    /// A non-empty `NO_COLOR` drops every colour and keeps the modifiers,
    /// so the selection and Unread rows stay visible.
    pub fn from_env() -> Self {
        let theme = Self {
            unread_marker: Style::new().fg(Color::Cyan),
            unread: Style::new().add_modifier(Modifier::BOLD),
            date: Style::new().fg(Color::DarkGray),
            selected: Style::new().add_modifier(Modifier::REVERSED),
        };
        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            theme.without_colours()
        } else {
            theme
        }
    }

    fn without_colours(self) -> Self {
        let strip = |style: Style| Style {
            fg: None,
            bg: None,
            underline_color: None,
            ..style
        };
        Self {
            unread_marker: strip(self.unread_marker),
            unread: strip(self.unread),
            date: strip(self.date),
            selected: strip(self.selected),
        }
    }
}
