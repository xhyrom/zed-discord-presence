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

mod fields;
pub use fields::ActivityFields;

use crate::{
    config::Configuration, document::Document, languages::get_language, util::Placeholders,
};

#[derive(Debug, Clone)]
pub struct ActivityManager;

impl ActivityManager {
    pub fn build_activity_fields(
        doc: Option<&Document>,
        config: &Configuration,
        workspace: &str,
    ) -> ActivityFields {
        let placeholders = Placeholders::new(doc, config, workspace);

        let activity = if let Some(doc) = doc {
            let language = get_language(doc).to_lowercase();
            config.languages.get(&language).unwrap_or(&config.activity)
        } else {
            &config.activity
        };

        ActivityFields::new(
            activity.state.as_ref(),
            activity.details.as_ref(),
            activity.large_image.as_ref(),
            activity.large_text.as_ref(),
            activity.small_image.as_ref(),
            activity.small_text.as_ref(),
        )
        .resolve_placeholders(&placeholders)
    }

    pub fn build_idle_activity_fields(config: &Configuration, workspace: &str) -> ActivityFields {
        let placeholders = Placeholders::new(None, config, workspace);

        ActivityFields::new(
            config.idle.state.as_ref(),
            config.idle.details.as_ref(),
            config.idle.large_image.as_ref(),
            config.idle.large_text.as_ref(),
            config.idle.small_image.as_ref(),
            config.idle.small_text.as_ref(),
        )
        .resolve_placeholders(&placeholders)
    }
}
