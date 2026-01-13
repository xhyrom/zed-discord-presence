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

#[derive(Debug)]
pub struct PresenceService {
    state: Arc<AppState>,
    idle_manager: IdleManager,
}

impl PresenceService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            idle_manager: IdleManager::new(),
        }
    }

    pub async fn update_presence(&self, doc: Option<Document>) -> Result<()> {
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

        self.set_discord_activity(activity_fields, git_url).await?;

        Ok(())
    }

    pub async fn initialize_discord(&self, application_id: &str) -> Result<()> {
        let mut discord = self.state.discord.lock().await;
        discord.create_client(application_id)?;
        discord.connect().await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        let discord = self.state.discord.lock().await;
        discord.kill().await?;
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
        let discord = self.state.discord.lock().await;
        let (state, details, large_image, large_text, small_image, small_text) =
            activity_fields.into_tuple();

        discord
            .change_activity(
                state,
                details,
                large_image,
                large_text,
                small_image,
                small_text,
                git_url,
            )
            .await?;

        Ok(())
    }

    async fn reset_idle_timeout(&self) -> Result<()> {
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
            )
            .await;

        Ok(())
    }
}
