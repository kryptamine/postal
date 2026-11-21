use crate::{inbox_view::InboxView, screen::Screen};

/// What the main screen is: the Message List and its Preview as the Cache
/// reads them, and what shows over and beside them.
pub struct Ui {
    pub view: InboxView,
    pub screen: Screen,
}
