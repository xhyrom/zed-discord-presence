/*
 * This file is part of discord-presence. Extension for Zed that adds support for Discord Rich Presence using LSP.
 *
 * Copyright (c) 2024 Steinhübl
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

use crate::{
    activity::ActivityManager, config::PresenceConfig, document::Document, error::Result,
    idle::IdleManager, service::AppState,
};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::{debug, info, warn};

/// First wait between background connect attempts; doubles after each failure.
const BACKGROUND_CONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
/// Longest wait between background connect attempts.
const BACKGROUND_CONNECT_MAX_DELAY: Duration = Duration::from_secs(30);

/// Leading-edge throttle state for presence updates (see `update_presence`).
#[derive(Debug, Default)]
struct Throttle {
    /// When the last update was dispatched.
    last_dispatch: Option<Instant>,
    /// Send scheduled for the end of the current window, while it waits.
    trailing: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
pub struct PresenceService {
    state: Arc<AppState>,
    idle_manager: Arc<IdleManager>,
    pending_doc: Arc<Mutex<Option<Document>>>,
    update_throttle: Duration,
    throttle: Arc<StdMutex<Throttle>>,
}

impl PresenceService {
    pub fn new(state: Arc<AppState>, config: PresenceConfig) -> Self {
        let idle_manager = Arc::new(IdleManager::new(Arc::clone(&state.shutting_down)));

        Self {
            state,
            idle_manager,
            pending_doc: Arc::new(Mutex::new(None)),
            update_throttle: config.update_throttle,
            throttle: Arc::default(),
        }
    }

    /// Leading-edge throttle: if nothing was dispatched within the throttle
    /// window, the update is sent at once; otherwise the latest pending update
    /// is sent when the window ends.
    pub async fn update_presence(&self, doc: Option<Document>) -> Result<()> {
        // The latest event always wins: the next send takes this document.
        *self.pending_doc.lock().await = doc;

        let now = Instant::now();
        let mut throttle = self.throttle.lock().unwrap_or_else(PoisonError::into_inner);
        let since_last = throttle
            .last_dispatch
            .map(|last| now.saturating_duration_since(last));

        match since_last {
            Some(elapsed) if elapsed < self.update_throttle => {
                // Inside the window: one trailing send at its end picks up
                // whatever is pending by then.
                if throttle.trailing.is_none() {
                    let delay = self.update_throttle.saturating_sub(elapsed);
                    throttle.trailing = Some(self.spawn_send(Some(delay)));
                }
            }
            _ => {
                // Leading edge: send now.
                throttle.last_dispatch = Some(now);
                drop(throttle);
                self.spawn_send(None);
            }
        }

        Ok(())
    }

    /// Spawns one send of the latest pending document: right away, or after
    /// `trailing_delay` for the send at the end of a throttle window, which
    /// then starts a new window.
    fn spawn_send(&self, trailing_delay: Option<Duration>) -> tokio::task::JoinHandle<()> {
        let state = Arc::clone(&self.state);
        let pending_doc = Arc::clone(&self.pending_doc);
        let throttle = Arc::clone(&self.throttle);
        let idle_manager = Arc::clone(&self.idle_manager);

        tokio::spawn(async move {
            if let Some(delay) = trailing_delay {
                tokio::time::sleep(delay).await;
                let mut window = throttle.lock().unwrap_or_else(PoisonError::into_inner);
                window.trailing = None;
                window.last_dispatch = Some(Instant::now());
            }

            // Take it in its own statement: in edition 2021 a lock guard in an
            // `if let` scrutinee lives until the end of the block, which would
            // make every update_presence call wait for the whole Discord send.
            let doc = pending_doc.lock().await.take();
            let Some(doc) = doc else {
                return;
            };

            let update_start = std::time::Instant::now();
            if let Err(e) = Self::perform_update_internal(&state, &idle_manager, Some(doc)).await {
                warn!("Failed to update Discord presence: {}", e);
            } else {
                info!(
                    "Discord presence updated in {:.1}ms (throttle delay: {:.0}ms)",
                    update_start.elapsed().as_secs_f64() * 1000.0,
                    trailing_delay.unwrap_or_default().as_secs_f64() * 1000.0
                );
            }
        })
    }

    async fn perform_update_internal(
        state: &Arc<AppState>,
        idle_manager: &IdleManager,
        doc: Option<Document>,
    ) -> Result<()> {
        if state.is_shutting_down() {
            debug!("Skipping presence update because shutdown is in progress");
            return Ok(());
        }

        // Store the last document for idle use
        {
            let mut last_doc = state.last_document.lock().await;
            (*last_doc).clone_from(&doc);
        }

        // Reset idle timeout if document changed. The shared manager aborts the
        // previous timer, so only the most recent update can trigger the idle action.
        if doc.is_some() {
            let workspace_name = {
                let workspace = state.workspace.lock().await;
                workspace.name().to_string()
            };
            idle_manager
                .reset_timeout(
                    Arc::clone(&state.discord),
                    Arc::clone(&state.config),
                    Arc::clone(&state.git_remote_url),
                    Arc::clone(&state.git_branch),
                    Arc::clone(&state.last_document),
                    workspace_name,
                )
                .await;
        }

        if state.is_shutting_down() {
            debug!("Skipping Discord activity update because shutdown started mid-update");
            return Ok(());
        }

        // Build and set activity
        let activity_fields = Self::build_activity_fields_internal(state, doc.as_ref()).await?;
        let git_url = Self::get_git_url_if_enabled_internal(state).await?;

        // Set Discord activity
        if state.is_shutting_down() {
            debug!("Skipping Discord activity update because shutdown is in progress");
            return Ok(());
        }

        let mut discord = state.discord.lock().await;

        debug!("Sending presence update");
        discord
            .change_activity_with_reconnect(activity_fields, git_url)
            .await?;

        Ok(())
    }

    async fn build_activity_fields_internal(
        state: &Arc<AppState>,
        doc: Option<&Document>,
    ) -> Result<crate::activity::ActivityFields> {
        let config = state.config.lock().await;
        let workspace = state.workspace.lock().await;
        let git_branch = state.git_branch.lock().await.clone();
        let git_remote_url = state.git_remote_url.lock().await.clone();

        Ok(ActivityManager::build_activity_fields(
            doc,
            &config,
            workspace.name(),
            workspace.path().unwrap_or(""),
            git_branch,
            git_remote_url.as_deref(),
        ))
    }

    async fn get_git_url_if_enabled_internal(state: &Arc<AppState>) -> Result<Option<String>> {
        let config = state.config.lock().await;
        let git_remote_url = state.git_remote_url.lock().await.clone();

        let workspace_override = {
            let workspace = state.workspace.lock().await;
            config
                .find_workspace_override(workspace.path().unwrap_or(""), git_remote_url.as_deref())
                .map(|ov| ov.effective_git_integration(config.git_integration))
        };

        let git_enabled = workspace_override.unwrap_or(config.git_integration);

        if git_enabled {
            Ok(git_remote_url)
        } else {
            Ok(None)
        }
    }

    pub async fn initialize_discord(&self, application_id: &str) -> Result<()> {
        if self.state.is_shutting_down() {
            debug!("Skipping Discord initialization because shutdown is in progress");
            return Ok(());
        }

        // Only create the client: connecting is left to
        // start_background_connect and to the first send, so initialize never
        // waits for Discord.
        let mut discord = self.state.discord.lock().await;
        discord.create_client(application_id)?;
        Ok(())
    }

    /// Connects to Discord in the background, retrying until it is reachable
    /// (for example when Zed starts before Discord), then sends the latest
    /// presence, if an update already happened.
    pub fn start_background_connect(&self) {
        let state = Arc::clone(&self.state);
        let idle_manager = Arc::clone(&self.idle_manager);

        tokio::spawn(async move {
            let mut delay = BACKGROUND_CONNECT_INITIAL_DELAY;

            loop {
                if state.is_shutting_down() {
                    return;
                }

                let connected = {
                    let discord = state.discord.lock().await;
                    discord.is_connected() || discord.connect().await.is_ok()
                };
                if connected {
                    break;
                }

                debug!("Discord not reachable yet, retrying in {:?}", delay);
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(BACKGROUND_CONNECT_MAX_DELAY);
            }

            // Updates made while Discord was unreachable may have failed:
            // send the latest one now. An already sent one is deduplicated.
            let doc = state.last_document.lock().await.clone();
            if doc.is_none() {
                info!("Discord is reachable");
                return;
            }

            info!("Discord is reachable; resending the latest presence");
            if let Err(e) = Self::perform_update_internal(&state, &idle_manager, doc).await {
                warn!("Failed to send the latest presence after connecting: {}", e);
            }
        });
    }

    pub async fn shutdown(&self) -> Result<()> {
        if !self.state.mark_shutting_down() {
            debug!("Presence service shutdown already in progress");
            self.idle_manager.cancel_timeout().await;
            return Ok(());
        }

        self.idle_manager.cancel_timeout().await;

        let mut discord = self.state.discord.lock().await;
        let mut first_error = None;

        if let Err(error) = discord.clear_activity().await {
            warn!(
                "Failed to clear Discord activity during shutdown: {}",
                error
            );
            first_error = Some(error);
        }

        if let Err(error) = discord.kill().await {
            warn!("Failed to close Discord IPC during shutdown: {}", error);
            if first_error.is_none() {
                first_error = Some(error);
            }
        }

        if let Some(error) = first_error {
            return Err(error);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Absolute workspace root for test documents.
    ///
    /// `Url::from_file_path` rejects relative paths, and no single path literal
    /// is absolute on both Windows (needs a drive prefix) and Unix (needs a
    /// leading `/`), so "C:/test" made every URL built from it fail on Unix.
    fn test_workspace_root() -> std::path::PathBuf {
        if cfg!(windows) {
            std::path::PathBuf::from(r"C:\test")
        } else {
            std::path::PathBuf::from("/test")
        }
    }

    #[tokio::test]
    async fn test_shutdown_cancels_idle_timeout_and_marks_state() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());

        service.shutdown().await.unwrap();

        assert!(state.is_shutting_down());
        assert!(!service.idle_manager.has_timeout().await);
    }

    #[tokio::test]
    async fn test_shutdown_is_idempotent() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(state, PresenceConfig::default());

        assert!(service.shutdown().await.is_ok());
        assert!(service.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_update_presence_is_ignored_during_shutdown() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());

        assert!(state.mark_shutting_down());
        assert!(service.update_presence(None).await.is_ok());

        let last_document = state.last_document.lock().await;
        assert!(last_document.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn test_rapid_updates_send_the_first_at_once_and_the_latest_at_window_end() {
        use tower_lsp::lsp_types::Url;

        let state = Arc::new(AppState::new());
        let config = PresenceConfig::default();
        let service = PresenceService::new(Arc::clone(&state), config);

        let workspace_root = test_workspace_root();

        // Call update_presence 5 times rapidly (simulates rapid file switches)
        for i in 0..5 {
            let file_path = workspace_root.join(format!("file{}.rs", i));
            let url = Url::from_file_path(&file_path).unwrap();
            let doc = Document::new(&url, &workspace_root, None);
            assert!(service.update_presence(Some(doc)).await.is_ok());
            // Small delay between calls, well inside the 2 s throttle window
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Wait past the end of the throttle window
        tokio::time::sleep(tokio::time::Duration::from_millis(2_500)).await;

        // Verify the final document stored in state (should be last one)
        let last_doc = state.last_document.lock().await;
        assert!(last_doc.is_some());
        let last_filename = last_doc.as_ref().unwrap().get_filename().unwrap();
        assert_eq!(
            last_filename, "file4.rs",
            "Latest document should be stored"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_latest_document_sent_when_the_window_ends() {
        use tower_lsp::lsp_types::Url;

        let state = Arc::new(AppState::new());
        let config = PresenceConfig::default();
        let service = PresenceService::new(Arc::clone(&state), config);

        let workspace_root = test_workspace_root();

        // Send doc1
        let file_path1 = workspace_root.join("file1.rs");
        let url1 = Url::from_file_path(&file_path1).unwrap();
        let doc1 = Document::new(&url1, &workspace_root, None);
        assert!(service.update_presence(Some(doc1.clone())).await.is_ok());

        // Quickly send doc2 (inside the throttle window)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let file_path2 = workspace_root.join("file2.rs");
        let url2 = Url::from_file_path(&file_path2).unwrap();
        let doc2 = Document::new(&url2, &workspace_root, None);
        assert!(service.update_presence(Some(doc2.clone())).await.is_ok());

        // Wait past the end of the throttle window
        tokio::time::sleep(tokio::time::Duration::from_millis(2_500)).await;

        // Verify the last document stored should be doc2
        let last_document = state.last_document.lock().await;
        assert!(last_document.is_some());
        let last_filename = last_document.as_ref().unwrap().get_filename().unwrap();
        assert_eq!(
            last_filename, "file2.rs",
            "Latest document should be sent when the window ends"
        );
    }

    /// In-memory log sink used to count events emitted by background tasks.
    #[derive(Clone, Default)]
    struct LogCapture(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for LogCapture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl LogCapture {
        fn count(&self, needle: &str) -> usize {
            String::from_utf8_lossy(&self.0.lock().unwrap())
                .matches(needle)
                .count()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_spaced_updates_fire_idle_action_only_once() {
        use tower_lsp::lsp_types::Url;

        // Every idle action ends with this warning in tests, because no Discord
        // client exists, so counting it counts how many idle timers fired.
        const IDLE_FIRED: &str = "from idle task";

        crate::test_support::keep_callsites_interested();
        let capture = LogCapture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());
        let workspace_root = test_workspace_root();

        // Three updates spaced beyond the throttle window, so each one reaches
        // perform_update_internal and re-arms the idle timer.
        for i in 0..3 {
            let url = Url::from_file_path(workspace_root.join(format!("file{i}.rs"))).unwrap();
            let doc = Document::new(&url, &workspace_root, None);
            assert!(service.update_presence(Some(doc)).await.is_ok());
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        assert_eq!(
            capture.count(IDLE_FIRED),
            0,
            "No idle action before the timeout"
        );

        // Move past the default 300 s idle timeout, then let woken tasks run.
        tokio::time::advance(Duration::from_secs(301)).await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        assert_eq!(
            capture.count(IDLE_FIRED),
            1,
            "Only the timer armed by the latest update should fire"
        );
    }

    fn capture_logs() -> (LogCapture, tracing::subscriber::DefaultGuard) {
        crate::test_support::keep_callsites_interested();
        let capture = LogCapture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .finish();
        (capture, tracing::subscriber::set_default(subscriber))
    }

    #[tokio::test(start_paused = true)]
    async fn test_initialize_discord_does_not_wait_for_discord() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());
        let started = tokio::time::Instant::now();

        assert!(service.initialize_discord("0").await.is_ok());

        assert_eq!(
            started.elapsed(),
            Duration::ZERO,
            "No connect attempt or retry delay inside initialize"
        );
        let discord = state.discord.lock().await;
        assert!(discord.has_client());
        assert!(!discord.is_connected());
    }

    #[tokio::test(start_paused = true)]
    async fn test_first_update_after_a_failed_connect_is_sent_once_discord_is_reachable() {
        use tower_lsp::lsp_types::Url;

        // Logged right before every activity send attempt.
        const SEND_ATTEMPT: &str = "Sending presence update";
        const RESEND_AFTER_CONNECT: &str = "Discord is reachable; resending the latest presence";

        let (capture, _guard) = capture_logs();
        // No Discord client: every connect attempt fails, like a closed Discord.
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());
        service.start_background_connect();

        let workspace_root = test_workspace_root();
        let url = Url::from_file_path(workspace_root.join("a.rs")).unwrap();
        let doc = Document::new(&url, &workspace_root, None);
        assert!(service.update_presence(Some(doc)).await.is_ok());
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(capture.count(SEND_ATTEMPT), 1, "The update itself failed");
        assert_eq!(capture.count(RESEND_AFTER_CONNECT), 0);

        // Discord becomes reachable.
        state.discord.lock().await.set_connected_for_test(true);
        tokio::time::sleep(BACKGROUND_CONNECT_MAX_DELAY).await;

        assert_eq!(capture.count(RESEND_AFTER_CONNECT), 1);
        assert_eq!(
            capture.count(SEND_ATTEMPT),
            2,
            "The latest presence is sent again once connected"
        );
    }

    fn test_doc(name: &str) -> Document {
        let workspace_root = test_workspace_root();
        let url = tower_lsp::lsp_types::Url::from_file_path(workspace_root.join(name)).unwrap();
        Document::new(&url, &workspace_root, None)
    }

    /// File name of the document the last dispatched update carried.
    async fn last_sent_file(state: &AppState) -> Option<String> {
        state
            .last_document
            .lock()
            .await
            .as_ref()
            .and_then(|doc| doc.get_filename().ok())
    }

    #[tokio::test(start_paused = true)]
    async fn test_first_update_is_sent_immediately() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());

        assert!(service
            .update_presence(Some(test_doc("a.rs")))
            .await
            .is_ok());
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_eq!(last_sent_file(&state).await.as_deref(), Some("a.rs"));
    }

    #[tokio::test(start_paused = true)]
    async fn test_burst_sends_at_most_one_update_per_window_with_the_latest_last() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());
        let start = Instant::now();
        let mut sends: Vec<(Duration, String)> = Vec::new();

        // An event every 100 ms for 5 s, then 3 s of quiet.
        for i in 0..80 {
            if i < 50 {
                assert!(service
                    .update_presence(Some(test_doc(&format!("file{i}.rs"))))
                    .await
                    .is_ok());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;

            let sent = last_sent_file(&state).await;
            if let Some(file) = sent {
                if sends.last().map(|(_, last)| last) != Some(&file) {
                    sends.push((start.elapsed(), file));
                }
            }
        }

        assert_eq!(
            sends.first().map(|(_, file)| file.as_str()),
            Some("file0.rs")
        );
        assert!(
            sends[0].0 <= Duration::from_millis(100),
            "The first one is immediate"
        );
        for pair in sends.windows(2) {
            let gap = pair[1].0.saturating_sub(pair[0].0);
            assert!(gap >= Duration::from_millis(1_900), "Sends {gap:?} apart");
        }
        assert_eq!(
            sends.last().map(|(_, file)| file.as_str()),
            Some("file49.rs"),
            "The latest content goes out last"
        );
        assert!(sends.len() <= 4, "At most one send per 2 s: {sends:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn test_switch_after_a_quiet_window_is_sent_immediately() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state), PresenceConfig::default());
        assert!(service
            .update_presence(Some(test_doc("a.rs")))
            .await
            .is_ok());

        tokio::time::sleep(Duration::from_millis(2_500)).await;
        assert!(service
            .update_presence(Some(test_doc("b.rs")))
            .await
            .is_ok());
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_eq!(last_sent_file(&state).await.as_deref(), Some("b.rs"));
    }
}
