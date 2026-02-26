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
    activity::ActivityManager, document::Document, error::Result, idle::IdleManager,
    service::AppState,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};

const SHUTDOWN_CLEAR_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct PresenceService {
    state: Arc<AppState>,
    idle_manager: IdleManager,
    is_shutting_down: Arc<AtomicBool>,
}

impl PresenceService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            idle_manager: IdleManager::new(),
            is_shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn update_presence(&self, doc: Option<Document>) -> Result<()> {
        if self.is_shutting_down.load(Ordering::SeqCst) {
            debug!("Skipping presence update because shutdown is in progress");
            return Ok(());
        }

        // Store the last document for idle use
        {
            let mut last_doc = self.state.last_document.lock().await;
            (*last_doc).clone_from(&doc);
        }

        // Reset idle timeout if document changed
        if doc.is_some() {
            self.reset_idle_timeout().await?;
        }

        // Build and set activity
        let activity_fields = self.build_activity_fields(doc.as_ref()).await?;
        let git_url = self.get_git_url_if_enabled().await?;

        if self.is_shutting_down.load(Ordering::SeqCst) {
            debug!("Skipping presence update because shutdown started during processing");
            return Ok(());
        }

        self.set_discord_activity(activity_fields, git_url).await?;

        Ok(())
    }

    pub async fn initialize_discord(&self, application_id: &str) -> Result<()> {
        self.is_shutting_down.store(false, Ordering::SeqCst);

        let mut discord = self.state.discord.lock().await;
        discord.create_client(application_id)?;
        discord.connect_with_retry().await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        if self.is_shutting_down.swap(true, Ordering::SeqCst) {
            debug!("Shutdown already in progress or completed");
            return Ok(());
        }

        self.idle_manager.cancel_timeout().await;

        let mut discord = self.state.discord.lock().await;

        match timeout(
            SHUTDOWN_CLEAR_TIMEOUT,
            discord.clear_activity_with_reconnect(),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!("Failed to clear activity during shutdown: {}", e);
            }
            Err(_) => {
                warn!(
                    "Timed out while clearing activity during shutdown after {:?}",
                    SHUTDOWN_CLEAR_TIMEOUT
                );
            }
        }

        if let Err(e) = discord.kill().await {
            warn!("Failed to close Discord IPC during shutdown: {}", e);
        }

        Ok(())
    }

    async fn build_activity_fields(
        &self,
        doc: Option<&Document>,
    ) -> Result<crate::activity::ActivityFields> {
        let config = self.state.config.lock().await;
        let workspace = self.state.workspace.lock().await;
        let git_branch = self.state.git_branch.lock().await.clone();

        Ok(ActivityManager::build_activity_fields(
            doc,
            &config,
            workspace.name(),
            git_branch,
        ))
    }

    async fn get_git_url_if_enabled(&self) -> Result<Option<String>> {
        let config = self.state.config.lock().await;

        if config.git_integration {
            let git_remote_url = self.state.git_remote_url.lock().await;
            Ok(git_remote_url.clone())
        } else {
            Ok(None)
        }
    }

    async fn set_discord_activity(
        &self,
        activity_fields: crate::activity::ActivityFields,
        git_url: Option<String>,
    ) -> Result<()> {
        if self.is_shutting_down.load(Ordering::SeqCst) {
            debug!("Skipping activity update because shutdown is in progress");
            return Ok(());
        }

        let mut discord = self.state.discord.lock().await;

        if self.is_shutting_down.load(Ordering::SeqCst) {
            debug!("Skipping activity update because shutdown started before Discord write");
            return Ok(());
        }

        discord
            .change_activity_with_reconnect(activity_fields, git_url)
            .await?;

        Ok(())
    }

    async fn reset_idle_timeout(&self) -> Result<()> {
        if self.is_shutting_down.load(Ordering::SeqCst) {
            debug!("Skipping idle timeout reset because shutdown is in progress");
            return Ok(());
        }

        let workspace_name = {
            let workspace = self.state.workspace.lock().await;
            workspace.name().to_string()
        };
        self.idle_manager
            .reset_timeout(
                Arc::clone(&self.state.discord),
                Arc::clone(&self.state.config),
                Arc::clone(&self.state.git_remote_url),
                Arc::clone(&self.state.git_branch),
                Arc::clone(&self.state.last_document),
                workspace_name,
                Arc::clone(&self.is_shutting_down),
            )
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::AppState;
    use std::path::Path;
    use std::sync::Arc;
    use tower_lsp::lsp_types::Url;

    fn test_document(path: &str) -> Document {
        let url = Url::parse(path).unwrap();
        let workspace_root = Path::new("/home/user/project");
        Document::new(&url, workspace_root, None)
    }

    #[tokio::test]
    async fn test_shutdown_suppresses_updates() {
        let app_state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&app_state));

        // Initial state
        assert!(!service.is_shutting_down.load(Ordering::SeqCst));

        // Initiate shutdown
        service.shutdown().await.unwrap();
        assert!(service.is_shutting_down.load(Ordering::SeqCst));

        // Try to update presence - should return Ok(()) immediately via the guard
        let result = service.update_presence(None).await;
        assert!(result.is_ok());

        // Try to reset idle timeout - should return Ok(()) immediately via the guard
        let result = service.reset_idle_timeout().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_double_shutdown_is_safe() {
        let app_state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&app_state));

        // First shutdown
        service.shutdown().await.unwrap();

        // Second shutdown should return Ok(()) via the swap guard
        let result = service.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_presence_when_shutting_down_does_not_replace_last_document() {
        let app_state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&app_state));

        let original_doc = test_document("file:///home/user/project/original.rs");
        {
            let mut last_doc = app_state.last_document.lock().await;
            *last_doc = Some(original_doc);
        }

        service.is_shutting_down.store(true, Ordering::SeqCst);

        let new_doc = test_document("file:///home/user/project/new.rs");
        let result = service.update_presence(Some(new_doc)).await;
        assert!(result.is_ok());

        let last_doc = app_state.last_document.lock().await;
        let filename = last_doc
            .as_ref()
            .expect("last document should remain set")
            .get_filename()
            .expect("filename should be available");
        assert_eq!(filename, "original.rs");
    }

    #[tokio::test]
    async fn test_update_presence_with_none_document_clears_last_document() {
        let app_state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&app_state));

        {
            let mut discord = app_state.discord.lock().await;
            discord.create_client("123456789012345678").unwrap();
        }

        {
            let mut last_doc = app_state.last_document.lock().await;
            *last_doc = Some(test_document("file:///home/user/project/file.rs"));
        }

        // This may fail if Discord is unavailable in test environment, but
        // last_document should still be updated before IPC write.
        let _ = service.update_presence(None).await;

        let last_doc = app_state.last_document.lock().await;
        assert!(last_doc.is_none());
    }
}
