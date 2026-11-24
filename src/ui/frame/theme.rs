use ratatui::style::{Color, Modifier, Style};

use crate::{
    domain::message::Message, render::icons::Icons, render::mail_colours::MailColours,
    render::mail_look::MailLook, render::marks::Marks, render::paint::Paint, render::rgb::Rgb,
};

/// The terminal's own palette: named ANSI colours and modifiers, no explicit
/// background. Muted text is `DIM`, since bright black vanishes in Solarized.
pub struct Theme {
    pub unread: Style,
    /// The Unread marker on a card.
    pub marker: Style,
    pub sender: Style,
    pub date: Style,
    pub subject: Style,
    /// A header label in the Preview: `From`, `Subject`, `Date`.
    pub field: Style,
    pub muted: Style,
    pub selected: Style,
    pub border: Style,
    /// A Settings tab shown and the row selected in it.
    pub chosen: Style,
    pub title: Style,
    /// A key on the hint line and in the `?` popup.
    pub key: Style,
    /// A hint mode label, apart from the blue links and selection.
    pub label: Style,
    pub warning: Style,
    pub error: Style,
    /// The characters a Query matched on a card; underlined under `NO_COLOR`.
    pub matched: Style,
    strong: Style,
    emphasis: Style,
    pub link: Style,
    code: Style,
    underline: Style,
    /// Whether the rendered View keeps the mail's colours.
    coloured: bool,
    /// Mail colours moved onto this terminal's background.
    pub mail: MailColours,
    pub icons: Icons,
}

impl Theme {
    /// A non-empty `NO_COLOR` drops every colour and keeps the modifiers,
    /// so the selection and Unread rows stay visible.
    pub fn from_env(background: Rgb, icons: Icons) -> Self {
        let coloured = std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty());
        let fg = |color| {
            if coloured {
                Style::new().fg(color)
            } else {
                Style::new()
            }
        };

        let bold = Style::new().add_modifier(Modifier::BOLD);
        let dim = Style::new().add_modifier(Modifier::DIM);
        Self {
            unread: bold,
            marker: fg(Color::Yellow),
            sender: fg(Color::Magenta),
            date: fg(Color::Cyan),
            subject: bold,
            field: fg(Color::Blue),
            muted: dim,
            selected: fg(Color::Blue).add_modifier(Modifier::REVERSED),
            border: dim,
            chosen: fg(Color::Yellow).add_modifier(Modifier::REVERSED | Modifier::BOLD),
            title: fg(Color::Blue).add_modifier(Modifier::BOLD),
            key: fg(Color::Yellow).add_modifier(Modifier::BOLD),
            label: fg(Color::Yellow).add_modifier(Modifier::REVERSED | Modifier::BOLD),
            warning: fg(Color::Yellow),
            error: fg(Color::Red),
            matched: if coloured {
                bold.fg(Color::Green)
            } else {
                bold.add_modifier(Modifier::UNDERLINED)
            },
            strong: bold,
            emphasis: Style::new().add_modifier(Modifier::ITALIC),
            link: fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
            code: fg(Color::Yellow),
            underline: Style::new().add_modifier(Modifier::UNDERLINED),
            coloured,
            mail: MailColours::new(background),
            icons,
        }
    }

    pub const fn mail_look(&self) -> MailLook {
        MailLook {
            colours: self.mail,
            icons: self.icons,
        }
    }

    /// The style of the sender of `message`: muted for the Account's own.
    pub const fn sender_of(&self, message: &Message) -> Style {
        if message.from_me {
            self.muted
        } else {
            self.sender
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
            (Marks::UNDERLINE, self.underline),
            (Marks::BORDER, self.border),
            (Marks::MUTED, self.muted),
            (Marks::HEADING, self.title),
        ]
        .into_iter()
        .filter(|&(mark, _)| marks.has(mark))
        .fold(Style::new(), |style, (_, patch)| style.patch(patch))
    }

    /// `marked`, in the mail's colours too unless `NO_COLOR` is set.
    pub fn painted(&self, marks: Marks, paint: Paint) -> Style {
        let style = self.marked(marks);
        if !self.coloured {
            return style;
        }

        let rgb = |Rgb { r, g, b }| Color::Rgb(r, g, b);
        let style = paint.fg.map_or(style, |fg| style.fg(rgb(fg)));
        paint.bg.map_or(style, |bg| style.bg(rgb(bg)))
    }
}
