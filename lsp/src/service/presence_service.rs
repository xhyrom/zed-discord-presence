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
use tracing::{debug, warn};

#[derive(Debug)]
pub struct PresenceService {
    state: Arc<AppState>,
    idle_manager: IdleManager,
}

impl PresenceService {
    pub fn new(state: Arc<AppState>) -> Self {
        let idle_manager = IdleManager::new(Arc::clone(&state.shutting_down));

        Self {
            state,
            idle_manager,
        }
    }

    pub async fn update_presence(&self, doc: Option<Document>) -> Result<()> {
        if self.state.is_shutting_down() {
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

        if self.state.is_shutting_down() {
            debug!("Skipping Discord activity update because shutdown started mid-update");
            return Ok(());
        }

        // Build and set activity
        let activity_fields = self.build_activity_fields(doc.as_ref()).await?;
        let git_url = self.get_git_url_if_enabled().await?;

        self.set_discord_activity(activity_fields, git_url).await?;

        Ok(())
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
            workspace.path().unwrap_or_else(|| workspace.name()),
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
        if self.state.is_shutting_down() {
            debug!("Skipping Discord activity update because shutdown is in progress");
            return Ok(());
        }

        let mut discord = self.state.discord.lock().await;

        discord
            .change_activity_with_reconnect(activity_fields, git_url)
            .await?;

        Ok(())
    }

    async fn reset_idle_timeout(&self) -> Result<()> {
        if self.state.is_shutting_down() {
            debug!("Skipping idle timeout reset because shutdown is in progress");
            return Ok(());
        }

        let workspace_path = {
            let workspace = self.state.workspace.lock().await;
            workspace
                .path()
                .unwrap_or_else(|| workspace.name())
                .to_string()
        };
        self.idle_manager
            .reset_timeout(
                Arc::clone(&self.state.discord),
                Arc::clone(&self.state.config),
                Arc::clone(&self.state.git_remote_url),
                Arc::clone(&self.state.git_branch),
                Arc::clone(&self.state.last_document),
                workspace_path,
            )
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_cancels_idle_timeout_and_marks_state() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state));

        service.reset_idle_timeout().await.unwrap();
        assert!(service.idle_manager.has_timeout().await);

        service.shutdown().await.unwrap();

        assert!(state.is_shutting_down());
        assert!(!service.idle_manager.has_timeout().await);
    }

    #[tokio::test]
    async fn test_shutdown_is_idempotent() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(state);

        assert!(service.shutdown().await.is_ok());
        assert!(service.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_update_presence_is_ignored_during_shutdown() {
        let state = Arc::new(AppState::new());
        let service = PresenceService::new(Arc::clone(&state));

        assert!(state.mark_shutting_down());
        assert!(service.update_presence(None).await.is_ok());

        let last_document = state.last_document.lock().await;
        assert!(last_document.is_none());
    }
}
