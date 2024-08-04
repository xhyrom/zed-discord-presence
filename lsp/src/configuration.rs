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

use serde_json::Value;

#[derive(Debug)]
pub struct Configuration {
    pub base_icons_url: String,

    pub state: String,
    pub details: String,

    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,

    pub git_integration: bool,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            base_icons_url: String::from("https://raw.githubusercontent.com/xhyrom/zed-discord-presence/feat/recognize-languages/assets/icons/"),
            state: String::from("Working on {filename}"),
            details: String::from("In {workspace}"),
            large_image: Some(String::from("{base_icons_url}/{language}.png")),
            large_text: Some(String::from("{language}")),
            small_image: Some(String::from("{base_icons_url}/zed.png")),
            small_text: Some(String::from("Zed")),
            git_integration: true,
        }
    }

    pub fn set(&mut self, initialization_options: Option<Value>) {
        if initialization_options.is_none() {
            return;
        }

        let initialization_options = initialization_options.unwrap();

        if let Some(base_icons_url) = initialization_options.get("base_icons_url") {
            self.base_icons_url = base_icons_url.as_str().unwrap().to_string();
        }

        if let Some(state) = initialization_options.get("state") {
            self.state = state.as_str().unwrap().to_string();
        }

        if let Some(details) = initialization_options.get("details") {
            self.details = details.as_str().unwrap().to_string();
        }

        if let Some(large_image) = initialization_options.get("large_image") {
            self.large_image = Some(large_image.as_str().unwrap().to_string())
        }

        if let Some(large_text) = initialization_options.get("large_text") {
            self.large_text = Some(large_text.as_str().unwrap().to_string())
        }

        if let Some(small_image) = initialization_options.get("small_image") {
            self.small_image = Some(small_image.as_str().unwrap().to_string())
        }

        if let Some(small_text) = initialization_options.get("small_text") {
            self.small_text = Some(small_text.as_str().unwrap().to_string())
        }

        if let Some(git_integration) = initialization_options.get("git_integration") {
            self.git_integration = git_integration.as_bool().unwrap_or(true);
        }
    }
}
