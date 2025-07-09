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

impl ActivityFields {
    pub fn new(
        state: Option<&String>,
        details: Option<&String>,
        large_image: Option<&String>,
        large_text: Option<&String>,
        small_image: Option<&String>,
        small_text: Option<&String>,
    ) -> Self {
        Self {
            state: state.cloned(),
            details: details.cloned(),
            large_image: large_image.cloned(),
            large_text: large_text.cloned(),
            small_image: small_image.cloned(),
            small_text: small_text.cloned(),
        }
    }

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

    #[allow(clippy::type_complexity)]
    pub fn into_tuple(
        self,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        (
            self.state,
            self.details,
            self.large_image,
            self.large_text,
            self.small_image,
            self.small_text,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_fields_creation() {
        let state = Some("Working on {filename}".to_string());
        let details = Some("In {workspace}".to_string());

        let fields = ActivityFields::new(&state, &details, &None, &None, &None, &None);

        assert_eq!(fields.state, Some("Working on {filename}".to_string()));
        assert_eq!(fields.details, Some("In {workspace}".to_string()));
    }
}
