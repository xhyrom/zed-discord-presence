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

use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use discord_rich_presence::{
    activity::{self, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

#[derive(Debug)]
pub struct Discord {
    client: Mutex<DiscordIpcClient>,
    start_timestamp: Duration,
}

impl Discord {
    pub fn new() -> Self {
        let discord_client = DiscordIpcClient::new("1263505205522337886")
            .expect("Failed to initialize Discord Ipc Client");
        let start_timestamp = SystemTime::now();
        let since_epoch = start_timestamp
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get duration since UNIX_EPOCH");

        Self {
            client: Mutex::new(discord_client),
            start_timestamp: since_epoch,
        }
    }

    pub fn connect(&self) {
        let mut client = self.get_client();
        let result = client.connect();
        result.unwrap();
    }

    pub fn kill(&self) {
        let mut client = self.get_client();
        let result = client.close();
        result.unwrap();
    }

    pub fn get_client(&self) -> MutexGuard<DiscordIpcClient> {
        return self.client.lock().expect("Failed to lock discord client");
    }

    pub fn change_activity(
        &self,
        state: String,
        details: String,
        large_image: Option<String>,
        large_text: Option<String>,
        small_image: Option<String>,
        small_text: Option<String>,
        git_remote_url: Option<String>,
    ) {
        let mut client = self.get_client();
        let timestamp: i64 = self.start_timestamp.as_millis() as i64;

        let mut assets = Assets::new();

        if let Some(large_image) = large_image.as_ref() {
            assets = assets.large_image(large_image);
        }

        if let Some(large_text) = large_text.as_ref() {
            assets = assets.large_text(large_text);
        }

        if let Some(small_image) = small_image.as_ref() {
            assets = assets.small_image(small_image);
        }

        if let Some(small_text) = small_text.as_ref() {
            assets = assets.small_text(small_text);
        }

        let mut buttons: Vec<Button> = Vec::new();

        if let Some(git_remote_url) = git_remote_url.as_ref() {
            buttons.push(Button::new("View Repository", git_remote_url));
        }

        client
            .set_activity(
                activity::Activity::new()
                    .assets(assets)
                    .state(state.as_str())
                    .details(details.as_str())
                    .timestamps(Timestamps::new().start(timestamp))
                    .buttons(buttons),
            )
            .unwrap_or_else(|_| {
                println!(
                    "Failed to set activity with state {} and details {}",
                    state, details
                )
            });
    }
}
