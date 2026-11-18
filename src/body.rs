use crate::preview::Preview;

/// Where one Message's Preview stands at one width and View.
pub enum Body<'a> {
    Rendered(&'a Preview),
    /// Asked for and on its way, or not yet asked for.
    Loading,
    /// The body cannot come for now; the placeholder said in its place.
    Unavailable(&'a str),
}
