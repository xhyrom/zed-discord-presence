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

use crate::config::activity::Activity;
use crate::util::Placeholders;

#[derive(Debug, Clone)]
pub struct ActivityFields {
    pub state: Option<String>,
    pub details: Option<String>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

impl From<&Activity> for ActivityFields {
    fn from(config: &Activity) -> Self {
        Self {
            state: config.state.clone(),
            details: config.details.clone(),
            large_image: config.large_image.clone(),
            large_text: config.large_text.clone(),
            small_image: config.small_image.clone(),
            small_text: config.small_text.clone(),
        }
    }
}

impl ActivityFields {
    pub fn resolve_placeholders(self, placeholders: &Placeholders) -> Self {
        Self {
            state: self.state.map(|s| placeholders.replace(&s)),
            details: self.details.map(|d| placeholders.replace(&d)),
            large_image: self.large_image.map(|img| placeholders.replace(&img)),
            large_text: self.large_text.map(|text| placeholders.replace(&text)),
            small_image: self.small_image.map(|img| placeholders.replace(&img)),
            small_text: self.small_text.map(|text| placeholders.replace(&text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_fields_creation() {
        let state = Some("Working on {filename}".to_string());
        let details = Some("In {workspace}".to_string());

        let activity = Activity {
            state: state.clone(),
            details: details.clone(),
            large_image: None,
            large_text: None,
            small_image: None,
            small_text: None,
        };
        let fields = ActivityFields::from(&activity);

        assert_eq!(fields.state, Some("Working on {filename}".to_string()));
        assert_eq!(fields.details, Some("In {workspace}".to_string()));
    }
}
