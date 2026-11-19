use std::{fmt::Write, time::SystemTime};

use ring::rand::{SecureRandom, SystemRandom};

/// The time in milliseconds, a dot, and 8 random bytes in hex: unique
/// enough to name a Draft's file or a Message without a counter.
pub fn unique_id() -> String {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut random = [0; 8];
    // Fails only on a broken system; the bytes stay zero, and the time
    // before them still tells ids apart.
    SystemRandom::new().fill(&mut random).ok();
    random.iter().fold(format!("{millis}."), |mut id, byte| {
        write!(id, "{byte:02x}").ok();
        id
    })
}
