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

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;

use crate::{
    activity::ActivityManager,
    config::{Configuration, IdleAction},
    discord::Discord,
};

#[derive(Debug)]
pub struct IdleManager {
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn reset_timeout(
        &self,
        discord: Arc<Mutex<Discord>>,
        config: Arc<Mutex<Configuration>>,
        git_remote_url: Arc<Mutex<Option<String>>>,
        workspace: String,
    ) {
        let mut handle_guard = self.handle.lock().await;

        // Cancel existing timeout
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }

        // Get timeout duration
        let timeout_duration = {
            let config_guard = config.lock().await;
            config_guard.idle.timeout
        };

        // Spawn new timeout task
        let handle = tokio::spawn(async move {
            time::sleep(timeout_duration).await;

            let config_guard = config.lock().await;
            let discord_guard = discord.lock().await;

            match config_guard.idle.action {
                IdleAction::ClearActivity => {
                    let _ = discord_guard.clear_activity().await; // Ignore errors in background task
                }
                IdleAction::ChangeActivity => {
                    let activity_fields =
                        ActivityManager::build_idle_activity_fields(&config_guard, &workspace);

                    let git_url = if config_guard.git_integration {
                        let git_guard = git_remote_url.lock().await;
                        git_guard.clone()
                    } else {
                        None
                    };

                    let (state, details, large_image, large_text, small_image, small_text) =
                        activity_fields.into_tuple();

                    let _ = discord_guard
                        .change_activity(
                            state,
                            details,
                            large_image,
                            large_text,
                            small_image,
                            small_text,
                            git_url,
                        )
                        .await; // Ignore errors in background task
                }
            }
        });

        *handle_guard = Some(handle);
    }
}
