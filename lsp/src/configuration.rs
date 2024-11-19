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

#[derive(Debug, PartialEq)]
pub enum RulesMode {
    Whitelist,
    Blacklist,
}

#[derive(Debug)]
pub struct Rules {
    pub mode: RulesMode,
    pub paths: Vec<String>,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            mode: RulesMode::Blacklist,
            paths: Vec::new(),
        }
    }
}

impl Rules {
    pub fn suitable(&self, path: &str) -> bool {
        let contains = self.paths.contains(&path.to_string());

        if self.mode == RulesMode::Blacklist {
            !contains
        } else {
            contains
        }
    }
}

#[derive(Debug)]
pub struct Configuration {
    pub application_id: String,
    pub base_icons_url: String,

    pub state: Option<String>,
    pub details: Option<String>,

    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,

    pub rules: Rules,

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
            application_id: String::from("1263505205522337886"),
            base_icons_url: String::from(
                "https://raw.githubusercontent.com/xhyrom/zed-discord-presence/main/assets/icons/",
            ),
            state: Some(String::from("Working on {filename}")),
            details: Some(String::from("In {workspace}")),
            large_image: Some(String::from("{base_icons_url}/{language}.png")),
            large_text: Some(String::from("{language:u}")),
            small_image: Some(String::from("{base_icons_url}/zed.png")),
            small_text: Some(String::from("Zed")),
            rules: Rules::default(),
            git_integration: true,
        }
    }

    pub fn set(&mut self, initialization_options: Option<Value>) {
        if let Some(options) = initialization_options {
            set_string!(self, options, application_id, "application_id");
            set_string!(self, options, base_icons_url, "base_icons_url");
            set_option!(self, options, state, "state");
            set_option!(self, options, details, "details");
            set_option!(self, options, large_image, "large_image");
            set_option!(self, options, large_text, "large_text");
            set_option!(self, options, small_image, "small_image");
            set_option!(self, options, small_text, "small_text");

            if let Some(rules) = options.get("rules") {
                self.rules.mode = rules.get("mode").and_then(|m| m.as_str()).map_or(
                    RulesMode::Blacklist,
                    |mode| match mode {
                        "whitelist" => RulesMode::Whitelist,
                        "blacklist" => RulesMode::Blacklist,
                        _ => RulesMode::Blacklist,
                    },
                );

                self.rules.paths =
                    rules
                        .get("paths")
                        .and_then(|p| p.as_array())
                        .map_or(Vec::new(), |paths| {
                            paths
                                .iter()
                                .filter_map(|p| p.as_str().map(|s| s.to_string()))
                                .collect()
                        });
            }

            if let Some(git_integration) = options.get("git_integration") {
                self.git_integration = git_integration.as_bool().unwrap_or(true);
            }
        }
    }
}
