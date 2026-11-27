use termlens::Screen;

/// Nothing on screen is still connecting or loading: a busy machine shows
/// those states long enough for a snapshot to catch them.
pub fn settled(screen: &Screen) -> bool {
    !screen.contains("connecting…") && !screen.contains("Loading")
}
