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

macro_rules! set_option {
    ($self:ident, $options:ident, $field:ident, $key:expr) => {
        if let Some(value) = $options.get($key) {
            $self.$field = if value.is_null() {
                None
            } else {
                Some(value.as_str().unwrap().to_string())
            };
        }
    };
}

macro_rules! set_string {
    ($self:ident, $options:ident, $field:ident, $key:expr) => {
        if let Some(value) = $options.get($key) {
            $self.$field = value.as_str().unwrap().to_string();
        }
    };
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
        if let Some(options) = initialization_options {
            set_string!(self, options, base_icons_url, "base_icons_url");
            set_string!(self, options, state, "state");
            set_string!(self, options, details, "details");
            set_option!(self, options, large_image, "large_image");
            set_option!(self, options, large_text, "large_text");
            set_option!(self, options, small_image, "small_image");
            set_option!(self, options, small_text, "small_text");

            if let Some(git_integration) = options.get("git_integration") {
                self.git_integration = git_integration.as_bool().unwrap_or(true);
            }
        }
    }
}
