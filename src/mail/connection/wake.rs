use std::time::{Duration, SystemTime};

use tokio::time::Instant;

const TICK: Duration = Duration::from_secs(1);

/// How far the wall clock may run ahead of the monotonic one in a tick.
const JUMP: Duration = Duration::from_secs(5);

/// Resolves once the machine wakes from sleep. The monotonic clock stops
/// while it sleeps and the wall clock does not, so a wake shows as a jump.
pub async fn woke() {
    let (mut wall, mut monotonic) = (SystemTime::now(), Instant::now());
    loop {
        tokio::time::sleep(TICK).await;
        let (wall_now, monotonic_now) = (SystemTime::now(), Instant::now());
        let walked = wall_now.duration_since(wall).unwrap_or_default();
        if walked > monotonic_now - monotonic + JUMP {
            return;
        }

        (wall, monotonic) = (wall_now, monotonic_now);
    }
}
