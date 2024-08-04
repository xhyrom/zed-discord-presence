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
    activity::{Activity, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

use crate::util;

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

    #[allow(clippy::too_many_arguments)]
    pub fn change_activity(
        &self,
        state: Option<String>,
        details: Option<String>,
        large_image: Option<String>,
        large_text: Option<String>,
        small_image: Option<String>,
        small_text: Option<String>,
        git_remote_url: Option<String>,
    ) {
        let mut client = self.get_client();
        let timestamp: i64 = self.start_timestamp.as_millis() as i64;

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

        client
            .set_activity(activity)
            .unwrap_or_else(|_| println!("Failed to set activity with activity"));
    }
}
