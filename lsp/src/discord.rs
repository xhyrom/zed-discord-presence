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

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};

use discord_rich_presence::{
    activity::{Activity, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use tracing::{debug, error, info, instrument};

use crate::{error::Result, util};

#[derive(Debug)]
pub struct Discord {
    client: Option<Mutex<DiscordIpcClient>>,
    start_timestamp: Duration,
}

impl Discord {
    pub fn new() -> Self {
        let start_timestamp = SystemTime::now();
        let since_epoch = start_timestamp
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get duration since UNIX_EPOCH");

        Self {
            client: None,
            start_timestamp: since_epoch,
        }
    }

    #[instrument(skip(self))]
    pub fn create_client(&mut self, application_id: &str) -> Result<()> {
        info!(
            "Creating Discord IPC client with app ID: {}",
            application_id
        );

        let discord_client = DiscordIpcClient::new(application_id);
        self.client = Some(Mutex::new(discord_client));

        debug!("Discord IPC client created successfully");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn connect(&self) -> Result<()> {
        debug!("Connecting to Discord IPC");

        let mut client = self.get_client().await?;
        client.connect().map_err(|e| {
            error!("Failed to connect to Discord IPC: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to connect to Discord IPC: {e}"))
        })?;

        info!("Successfully connected to Discord IPC");
        Ok(())
    }

    pub async fn kill(&self) -> Result<()> {
        debug!("Killing Discord IPC client");

        let mut client = self.get_client().await?;
        client.close().map_err(|e| {
            crate::error::PresenceError::Discord(format!("Failed to close Discord connection: {e}"))
        })?;

        Ok(())
    }

    pub async fn get_client(&self) -> Result<MutexGuard<'_, DiscordIpcClient>> {
        let client = self.client.as_ref().ok_or_else(|| {
            crate::error::PresenceError::Discord("Discord client not initialized".to_string())
        })?;

        Ok(client.lock().await)
    }

    #[instrument(skip(self))]
    pub async fn clear_activity(&self) -> Result<()> {
        debug!("Clearing Discord activity");

        let mut client = self.get_client().await?;
        client.clear_activity().map_err(|e| {
            error!("Failed to clear activity: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to clear activity: {e}"))
        })?;

        info!("Discord activity cleared");
        Ok(())
    }

    #[instrument(skip(self), fields(
        state = state.as_deref().unwrap_or("None"),
        details = details.as_deref().unwrap_or("None")
    ))]
    #[allow(clippy::too_many_arguments)]
    pub async fn change_activity(
        &self,
        state: Option<String>,
        details: Option<String>,
        large_image: Option<String>,
        large_text: Option<String>,
        small_image: Option<String>,
        small_text: Option<String>,
        git_remote_url: Option<String>,
    ) -> Result<()> {
        let mut client = self.get_client().await?;
        let timestamp: i64 = i64::try_from(self.start_timestamp.as_secs()).map_err(|e| {
            error!("Failed to convert timestamp: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to convert timestamp: {e}"))
        })?;

        let activity = Activity::new()
            .timestamps(Timestamps::new().start(timestamp))
            .buttons(
                git_remote_url
                    .as_ref()
                    .map(|url| vec![Button::new("View Repository", url)])
                    .unwrap_or_default(),
            );

        let activity = util::set_optional_field(activity, state.as_deref(), Activity::state);
        let activity = util::set_optional_field(activity, details.as_deref(), Activity::details);

        let assets = Assets::new();
        let assets = util::set_optional_field(assets, large_image.as_deref(), Assets::large_image);
        let assets = util::set_optional_field(assets, large_text.as_deref(), Assets::large_text);
        let assets = util::set_optional_field(assets, small_image.as_deref(), Assets::small_image);
        let assets = util::set_optional_field(assets, small_text.as_deref(), Assets::small_text);

        let activity = activity.assets(assets);

        client.set_activity(activity).map_err(|e| {
            error!("Failed to set activity: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to set activity: {e}"))
        })?;

        debug!("Discord activity updated successfully");
        Ok(())
    }
}
