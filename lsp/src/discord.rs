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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};

use discord_rich_presence::{
    activity::{Activity, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use tracing::{debug, error, info, instrument, warn};

use crate::{error::Result, util};

/// Maximum number of connection retries
const MAX_RETRIES: u32 = 5;
/// Initial delay between retries in milliseconds
const INITIAL_DELAY_MS: u64 = 500;
/// Maximum delay between retries in milliseconds
const MAX_DELAY_MS: u64 = 10_000;

#[derive(Debug)]
pub struct Discord {
    client: Option<Mutex<DiscordIpcClient>>,
    start_timestamp: Duration,
    connected: Arc<AtomicBool>,
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
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns whether the Discord IPC client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
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
            self.connected.store(false, Ordering::SeqCst);
            error!("Failed to connect to Discord IPC: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to connect to Discord IPC: {e}"))
        })?;

        self.connected.store(true, Ordering::SeqCst);
        info!("Successfully connected to Discord IPC");
        Ok(())
    }

    /// Connects to Discord IPC with exponential backoff retry.
    /// Will retry up to `MAX_RETRIES` times with increasing delays.
    #[instrument(skip(self))]
    pub async fn connect_with_retry(&self) -> Result<()> {
        let mut delay = Duration::from_millis(INITIAL_DELAY_MS);

        for attempt in 1..=MAX_RETRIES {
            match self.connect().await {
                Ok(()) => {
                    info!("Connected to Discord IPC on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        warn!(
                            "Connection attempt {}/{} failed: {}. Retrying in {:?}...",
                            attempt, MAX_RETRIES, e, delay
                        );
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_millis(MAX_DELAY_MS));
                    } else {
                        error!(
                            "Failed to connect to Discord after {} attempts: {}",
                            MAX_RETRIES, e
                        );
                        return Err(e);
                    }
                }
            }
        }

        Err(crate::error::PresenceError::Discord(
            "Failed to connect after all retries".into(),
        ))
    }

    /// Attempts to reconnect to Discord, closing any existing connection first.
    #[instrument(skip(self))]
    pub async fn reconnect(&self) -> Result<()> {
        info!("Attempting to reconnect to Discord IPC");
        self.connected.store(false, Ordering::SeqCst);

        // Try to close existing connection (ignore errors)
        let _ = self.kill().await;

        self.connect_with_retry().await
    }

    pub async fn kill(&self) -> Result<()> {
        debug!("Killing Discord IPC client");
        self.connected.store(false, Ordering::SeqCst);

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
        let timestamp: i64 = i64::try_from(self.start_timestamp.as_millis()).map_err(|e| {
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

    /// Changes activity with automatic reconnection on failure.
    /// If not connected, attempts to reconnect first.
    /// If activity update fails, marks connection as disconnected for future reconnection.
    #[allow(clippy::too_many_arguments)]
    pub async fn change_activity_with_reconnect(
        &self,
        state: Option<String>,
        details: Option<String>,
        large_image: Option<String>,
        large_text: Option<String>,
        small_image: Option<String>,
        small_text: Option<String>,
        git_remote_url: Option<String>,
    ) -> Result<()> {
        // If not connected, try to reconnect first
        if !self.is_connected() {
            warn!("Discord not connected, attempting reconnection...");
            if let Err(e) = self.reconnect().await {
                debug!("Reconnection failed: {}", e);
                return Err(e);
            }
        }

        // Try to update activity
        match self
            .change_activity(
                state.clone(),
                details.clone(),
                large_image.clone(),
                large_text.clone(),
                small_image.clone(),
                small_text.clone(),
                git_remote_url.clone(),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                // Connection may have dropped, mark as disconnected
                warn!("Activity update failed, marking as disconnected: {}", e);
                self.connected.store(false, Ordering::SeqCst);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_new_defaults() {
        let discord = Discord::new();
        assert!(!discord.is_connected());
        assert!(discord.client.is_none());
    }

    #[test]
    fn test_is_connected_default_false() {
        let discord = Discord::new();
        assert!(!discord.is_connected());
    }

    #[test]
    fn test_connected_state_can_be_changed() {
        let discord = Discord::new();
        assert!(!discord.is_connected());

        // Manually set connected to true
        discord.connected.store(true, Ordering::SeqCst);
        assert!(discord.is_connected());

        // Set back to false
        discord.connected.store(false, Ordering::SeqCst);
        assert!(!discord.is_connected());
    }

    #[test]
    fn test_retry_constants() {
        // Verify retry constants are reasonable
        assert_eq!(MAX_RETRIES, 5);
        assert_eq!(INITIAL_DELAY_MS, 500);
        assert_eq!(MAX_DELAY_MS, 10_000);

        // Verify exponential backoff calculation
        let mut delay = Duration::from_millis(INITIAL_DELAY_MS);
        let delays: Vec<u64> = (0..5)
            .map(|_| {
                let current = delay.as_millis() as u64;
                delay = (delay * 2).min(Duration::from_millis(MAX_DELAY_MS));
                current
            })
            .collect();

        assert_eq!(delays, vec![500, 1000, 2000, 4000, 8000]);
    }

    #[test]
    fn test_start_timestamp_is_set() {
        let discord = Discord::new();
        // Timestamp should be non-zero (set to current time)
        assert!(discord.start_timestamp.as_millis() > 0);
    }
}
