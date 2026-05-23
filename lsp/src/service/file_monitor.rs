use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveFile {
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct FileMonitor {
    last_active: Arc<Mutex<Option<ActiveFile>>>,
}

impl FileMonitor {
    pub fn new() -> Self {
        Self {
            last_active: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if active file has changed since last check
    pub async fn check_active_file(&self, current_uri: Option<String>) -> Option<ActiveFile> {
        let current = current_uri.map(|uri| ActiveFile { uri });
        let mut last = self.last_active.lock().await;

        if current == *last {
            None
        } else {
            last.clone_from(&current);
            current
        }
    }
}

impl Default for FileMonitor {
    fn default() -> Self {
        Self::new()
    }
}
