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
    activity::ActivityManager,
    config::PresenceConfig,
    document::Document,
    error::Result,
    idle::IdleManager,
    service::{AppState, FileMonitor},
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct PresenceService {
    state: Arc<AppState>,
    idle_manager: IdleManager,
    file_monitor: FileMonitor,
    debounce_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pending_doc: Arc<Mutex<Option<Document>>>,
    update_debounce: Duration,
}

impl PresenceService {
    pub fn new(state: Arc<AppState>, config: PresenceConfig) -> Self {
        let idle_manager = IdleManager::new(Arc::clone(&state.shutting_down));

        Self {
            state,
            idle_manager,
            file_monitor: FileMonitor::new(),
            debounce_handle: Arc::new(Mutex::new(None)),
            pending_doc: Arc::new(Mutex::new(None)),
            update_debounce: config.update_debounce,
        }
    }

    pub fn file_monitor(&self) -> &FileMonitor {
        &self.file_monitor
    }

    pub async fn update_presence(&self, doc: Option<Document>) -> Result<()> {
        // Store the document for deferred update
        *self.pending_doc.lock().await = doc;

        // Cancel any pending debounce timer
        if let Some(handle) = self.debounce_handle.lock().await.take() {
            handle.abort();
        }

        let state = Arc::clone(&self.state);
        let pending_doc = Arc::clone(&self.pending_doc);
        let debounce_handle = Arc::clone(&self.debounce_handle);
        let debounce_delay = self.update_debounce;

        // Spawn debounce task
        let task = tokio::spawn(async move {
            tokio::time::sleep(debounce_delay).await;

            // Get the latest pending doc (might have changed during debounce)
            if let Some(doc) = pending_doc.lock().await.take() {
                let update_start = std::time::Instant::now();
                if let Err(e) = Self::perform_update_internal(&state, Some(doc)).await {
                    warn!("Failed to update Discord presence: {}", e);
                } else {
                    let total_elapsed = update_start.elapsed();
                    info!(
                        "Discord presence updated in {:.1}ms (debounce delay: {:.0}ms)",
                        total_elapsed.as_secs_f64() * 1000.0,
                        debounce_delay.as_secs_f64() * 1000.0
                    );
                }
            }

            // Clear the handle
            *debounce_handle.lock().await = None;
        });

        *self.debounce_handle.lock().await = Some(task);
        Ok(())
    }

    async fn perform_update_internal(state: &Arc<AppState>, doc: Option<Document>) -> Result<()> {
        if state.is_shutting_down() {
            debug!("Skipping presence update because shutdown is in progress");
            return Ok(());
        }

        // Store the last document for idle use
        {
            let mut last_doc = state.last_document.lock().await;
            (*last_doc).clone_from(&doc);
        }

        // Reset idle timeout if document changed
        if doc.is_some() {
            let idle_manager = IdleManager::new(Arc::clone(&state.shutting_down));
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

        let mut discord = self.state.discord.lock().await;
        discord.create_client(application_id)?;
        discord.connect_with_retry().await?;
        Ok(())
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

    #[tokio::test]
    async fn test_rapid_updates_batched_into_single_discord_call() {
        use std::path::PathBuf;
        use tower_lsp::lsp_types::Url;

        let state = Arc::new(AppState::new());
        let config = PresenceConfig::default();
        let service = PresenceService::new(Arc::clone(&state), config);

        let workspace_root = PathBuf::from("C:/test");

        // Call update_presence 5 times rapidly (simulates rapid file switches)
        for i in 0..5 {
            let file_path = workspace_root.join(format!("file{}.rs", i));
            let url = Url::from_file_path(&file_path).unwrap();
            let doc = Document::new(&url, &workspace_root, None);
            assert!(service.update_presence(Some(doc)).await.is_ok());
            // Small delay between calls, but less than debounce window (500ms)
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Wait for debounce to settle (500ms + buffer)
        tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;

        // Verify the final document stored in state (should be last one)
        let last_doc = state.last_document.lock().await;
        assert!(last_doc.is_some());
        let last_filename = last_doc.as_ref().unwrap().get_filename().unwrap();
        assert_eq!(
            last_filename, "file4.rs",
            "Latest document should be stored"
        );
    }

    #[tokio::test]
    async fn test_latest_document_sent_after_debounce() {
        use std::path::PathBuf;
        use tower_lsp::lsp_types::Url;

        let state = Arc::new(AppState::new());
        let config = PresenceConfig::default();
        let service = PresenceService::new(Arc::clone(&state), config);

        let workspace_root = PathBuf::from("C:/test");

        // Send doc1
        let file_path1 = workspace_root.join("file1.rs");
        let url1 = Url::from_file_path(&file_path1).unwrap();
        let doc1 = Document::new(&url1, &workspace_root, None);
        assert!(service.update_presence(Some(doc1.clone())).await.is_ok());

        // Quickly send doc2 (before debounce expires)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let file_path2 = workspace_root.join("file2.rs");
        let url2 = Url::from_file_path(&file_path2).unwrap();
        let doc2 = Document::new(&url2, &workspace_root, None);
        assert!(service.update_presence(Some(doc2.clone())).await.is_ok());

        // Wait for debounce to settle
        tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;

        // Verify the last document stored should be doc2
        let last_document = state.last_document.lock().await;
        assert!(last_document.is_some());
        let last_filename = last_document.as_ref().unwrap().get_filename().unwrap();
        assert_eq!(
            last_filename, "file2.rs",
            "Latest document should be stored after debounce"
        );
    }
}
