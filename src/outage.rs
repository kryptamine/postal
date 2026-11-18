/// Why a connection is down for now and worth trying again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outage {
    /// The network: no route, refused, or the connection dropped.
    Offline,
    /// The server is up but turned Postal away: `[THROTTLED]`,
    /// `[UNAVAILABLE]`, `BYE`, bandwidth limits.
    Busy,
}
