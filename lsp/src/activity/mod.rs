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

use crate::{config::Configuration, document::Document, util::Placeholders};

#[derive(Debug, Clone)]
pub struct ActivityManager;

impl ActivityManager {
    pub fn build_activity_fields(
        doc: Option<&Document>,
        config: &Configuration,
        workspace: &str,
    ) -> ActivityFields {
        let placeholders = Placeholders::new(doc, config, workspace);

        ActivityFields::new(
            &config.state,
            &config.details,
            &config.large_image,
            &config.large_text,
            &config.small_image,
            &config.small_text,
        )
        .resolve_placeholders(&placeholders)
    }

    pub fn build_idle_activity_fields(config: &Configuration, workspace: &str) -> ActivityFields {
        let placeholders = Placeholders::new(None, config, workspace);

        ActivityFields::new(
            &config.idle.state,
            &config.idle.details,
            &config.idle.large_image,
            &config.idle.large_text,
            &config.idle.small_image,
            &config.idle.small_text,
        )
        .resolve_placeholders(&placeholders)
    }
}
