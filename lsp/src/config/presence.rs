use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct PresenceConfig {
    /// Debounce rapid file switches. Updates within this duration are batched.
    pub update_debounce: Duration,
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            update_debounce: Duration::from_millis(500),
        }
    }
}
