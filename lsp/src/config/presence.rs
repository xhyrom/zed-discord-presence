use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct PresenceConfig {
    /// Leading-edge throttle window: an update is sent at once if none was
    /// sent within this window, otherwise the latest one is sent when it ends.
    pub update_throttle: Duration,
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            update_throttle: Duration::from_secs(2),
        }
    }
}
