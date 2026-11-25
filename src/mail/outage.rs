/// Why a connection is down, to be opened afresh after the backoff.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outage {
    /// The network: no route, refused, or the connection dropped.
    Offline,
    /// The server is up but turned Postal away: `[THROTTLED]`,
    /// `[UNAVAILABLE]`, `BYE`, bandwidth limits.
    Busy,
    /// Anything else that broke the connection's work, as a refused command
    /// nothing handles: a fresh connection starts over from the server.
    Failed,
}
