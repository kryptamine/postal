use ratatui::style::{Color, Modifier, Style};

use crate::marks::Marks;

/// The terminal's own palette: named ANSI colours and modifiers, no explicit
/// background. Muted text is `DIM`, since bright black vanishes in Solarized.
pub struct Theme {
    pub unread: Style,
    pub muted: Style,
    pub selected: Style,
    pub border: Style,
    pub title: Style,
    /// A key on the hint line and in the `?` popup.
    pub key: Style,
    /// A hint mode label, apart from the blue links and selection.
    pub label: Style,
    pub warning: Style,
    strong: Style,
    emphasis: Style,
    link: Style,
    code: Style,
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
            key: Style::new().add_modifier(Modifier::BOLD),
            label: Style::new()
                .fg(Color::Yellow)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
            warning: Style::new().fg(Color::Yellow),
            strong: Style::new().add_modifier(Modifier::BOLD),
            emphasis: Style::new().add_modifier(Modifier::ITALIC),
            link: Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            code: Style::new().fg(Color::Yellow),
        };

        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            theme.without_colours()
        } else {
            theme
        }
    }

    /// The style of Preview text with `marks`; an image's alt text is muted.
    pub fn marked(&self, marks: Marks) -> Style {
        [
            (Marks::STRONG, self.strong),
            (Marks::EMPHASIS, self.emphasis),
            (Marks::LINK, self.link),
            (Marks::CODE, self.code),
            (Marks::ALT, self.muted),
        ]
        .into_iter()
        .filter(|&(mark, _)| marks.has(mark))
        .fold(Style::new(), |style, (_, patch)| style.patch(patch))
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
            key: strip(self.key),
            label: strip(self.label),
            warning: strip(self.warning),
            strong: strip(self.strong),
            emphasis: strip(self.emphasis),
            link: strip(self.link),
            code: strip(self.code),
        }
    }
}
