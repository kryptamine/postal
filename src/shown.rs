use crate::preview::Preview;

/// What the Preview pane shows for the selected Message.
pub enum Shown<'a> {
    Rendered(&'a Preview),
    /// Asked for and on its way, or not yet asked for.
    Loading,
    /// The body cannot come for now: `header` holds the row's header only,
    /// `why` the placeholder said in its place.
    Unavailable {
        header: &'a Preview,
        why: &'a str,
    },
}
