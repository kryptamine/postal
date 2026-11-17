use ratatui::style::{Color, Modifier, Style};

/// The terminal's own palette: named ANSI colours and modifiers, no explicit
/// background. Muted text is `DIM`, since bright black vanishes in Solarized.
pub struct Theme {
    pub unread: Style,
    pub muted: Style,
    pub selected: Style,
    pub border: Style,
    pub title: Style,
}

impl Theme {
    /// A non-empty `NO_COLOR` drops every colour and keeps the modifiers,
    /// so the selection and Unread rows stay visible.
    pub fn from_env() -> Self {
        let theme = Self {
            unread: Style::new().add_modifier(Modifier::BOLD),
            muted: Style::new().add_modifier(Modifier::DIM),
            selected: Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::REVERSED),
            border: Style::new().add_modifier(Modifier::DIM),
            title: Style::new().add_modifier(Modifier::BOLD),
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
            unread: strip(self.unread),
            muted: strip(self.muted),
            selected: strip(self.selected),
            border: strip(self.border),
            title: strip(self.title),
        }
    }
}
