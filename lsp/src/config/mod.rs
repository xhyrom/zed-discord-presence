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

pub mod activity;
mod idle;
mod rules;
mod update;

use std::collections::HashMap;

pub use idle::{Idle, IdleAction};
pub use rules::Rules;
use tracing::{debug, error, info, instrument};
use update::UpdateFromJson;

use serde_json::{Map, Value};

use crate::{config::activity::Activity, error::Result};

const DEFAULT_APP_ID: &str = "1263505205522337886";
const DEFAULT_ICONS_URL: &str =
    "https://raw.githubusercontent.com/xhyrom/zed-discord-presence/main/assets/icons/";

#[derive(Debug, Clone)]
pub struct Configuration {
    pub application_id: String,
    pub base_icons_url: String,
    pub activity: Activity,
    pub rules: Rules,
    pub idle: Idle,
    pub git_integration: bool,
    pub languages: HashMap<String, Activity>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            application_id: DEFAULT_APP_ID.to_string(),
            base_icons_url: DEFAULT_ICONS_URL.to_string(),
            activity: Activity::default(),
            rules: Rules::default(),
            idle: Idle::default(),
            git_integration: true,
            languages: HashMap::default(),
        }
    }
}

impl UpdateFromJson for Configuration {
    fn update_from_json(&mut self, json: &Value) -> Result<()> {
        if let Some(app_id) = json.get("application_id").and_then(Value::as_str) {
            self.application_id = app_id.to_string();
        }

        if let Some(icons_url) = json.get("base_icons_url").and_then(Value::as_str) {
            self.base_icons_url = icons_url.to_string();
        }

        self.activity.update_from_json(json)?;

        if let Some(rules) = json.get("rules") {
            self.rules.update_from_json(rules)?;
        }

        if let Some(idle) = json.get("idle") {
            self.idle.update_from_json(idle)?;
        }

        if let Some(git_integration) = json.get("git_integration") {
            self.git_integration = git_integration.as_bool().unwrap_or(true);
        }

        if let Some(languages) = json.get("languages") {
            for (key, value) in languages.as_object().unwrap_or(&Map::default()) {
                let mut activity = Activity::default();

                if let Err(e) = activity.update_from_json(value) {
                    error!("Failed to update config for {} language: {}", key, e);
                    continue;
                }

                self.languages.insert(key.to_owned(), activity);
            }
        }

        Ok(())
    }
}

impl Configuration {
    #[instrument(skip(self, options))]
    pub fn update(&mut self, options: Option<Value>) -> Result<()> {
        if let Some(options) = options {
            debug!("Updating configuration from provided options");
            self.update_from_json(&options)?;
            info!("Configuration updated successfully");
        } else {
            debug!("No configuration options provided, using defaults");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration() {
        let config = Configuration::default();
        assert_eq!(config.application_id, DEFAULT_APP_ID);
        assert_eq!(config.base_icons_url, DEFAULT_ICONS_URL);
        assert!(config.git_integration);
    }

    #[test]
    fn test_update_configuration() {
        let mut config = Configuration::default();
        let json = serde_json::json!({
            "application_id": "test_id",
            "base_icons_url": "http://example.com",
            "git_integration": false
        });

        config.update(Some(json)).unwrap();

        assert_eq!(config.application_id, "test_id");
        assert_eq!(config.base_icons_url, "http://example.com");
        assert!(!config.git_integration);
    }

    #[test]
    fn test_update_languages_configuration() {
        let mut config = Configuration::default();
        let json = serde_json::json!({
            "languages": {
                "rust": {
                    "state": "Coding Rust",
                    "details": "In Cargo project",
                    "git_integration": false
                },
                "python": {
                    "state": "Scripting Python"
                }
            }
        });

        config.update(Some(json)).unwrap();

        assert!(config.languages.contains_key("rust"));
        assert!(config.languages.contains_key("python"));

        let rust_cfg = config.languages.get("rust").unwrap();
        assert_eq!(rust_cfg.state.as_deref(), Some("Coding Rust"));
        assert_eq!(rust_cfg.details.as_deref(), Some("In Cargo project"));

        let py_cfg = config.languages.get("python").unwrap();
        assert_eq!(py_cfg.state.as_deref(), Some("Scripting Python"));
        assert_eq!(py_cfg.details.as_deref(), Some("In {workspace}"));
    }
}
