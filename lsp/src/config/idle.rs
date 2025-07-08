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

use crate::error::Result;

use super::update::{update_optional_string_field, UpdateFromJson};
use serde_json::Value;
use std::time::Duration;

const DEFAULT_IDLE_TIMEOUT: u64 = 300; // 5 minutes

#[derive(Debug, Clone, PartialEq)]
pub enum IdleAction {
    ClearActivity,
    ChangeActivity,
}

impl Default for IdleAction {
    fn default() -> Self {
        Self::ChangeActivity
    }
}

#[derive(Debug, Clone)]
pub struct Idle {
    pub timeout: Duration,
    pub action: IdleAction,
    pub state: Option<String>,
    pub details: Option<String>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

impl Default for Idle {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(DEFAULT_IDLE_TIMEOUT),
            action: IdleAction::default(),
            state: Some("Idling".to_string()),
            details: Some("In Zed".to_string()),
            large_image: Some("{base_icons_url}/zed.png".to_string()),
            large_text: Some("Zed".to_string()),
            small_image: Some("{base_icons_url}/idle.png".to_string()),
            small_text: Some("Idle".to_string()),
        }
    }
}

impl UpdateFromJson for Idle {
    fn update_from_json(&mut self, json: &Value) -> Result<()> {
        if let Some(timeout) = json.get("timeout").and_then(Value::as_u64) {
            self.timeout = Duration::from_secs(timeout);
        }

        if let Some(action) = json.get("action").and_then(Value::as_str) {
            self.action = match action {
                "clear_activity" => IdleAction::ClearActivity,
                "change_activity" => IdleAction::ChangeActivity,
                _ => IdleAction::default(),
            };
        }

        update_optional_string_field!(self, json, state, "state");
        update_optional_string_field!(self, json, details, "details");
        update_optional_string_field!(self, json, large_image, "large_image");
        update_optional_string_field!(self, json, large_text, "large_text");
        update_optional_string_field!(self, json, small_image, "small_image");
        update_optional_string_field!(self, json, small_text, "small_text");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_idle() {
        let idle = Idle::default();
        assert_eq!(idle.timeout, Duration::from_secs(DEFAULT_IDLE_TIMEOUT));
        assert_eq!(idle.action, IdleAction::ChangeActivity);
    }

    #[test]
    fn test_update_from_json() {
        let mut idle = Idle::default();
        let json = serde_json::json!({
            "timeout": 600,
            "action": "clear_activity",
            "state": "Custom Idle State",
            "details": null
        });

        idle.update_from_json(&json).unwrap();

        assert_eq!(idle.timeout, Duration::from_secs(600));
        assert_eq!(idle.action, IdleAction::ClearActivity);
        assert_eq!(idle.state, Some("Custom Idle State".to_string()));
        assert_eq!(idle.details, None);
    }
}
