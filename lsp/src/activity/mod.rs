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

fn resolve_workspace(workspace: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(workspace).join(".zed/settings.json"))
        .ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .and_then(|json| json["project_name"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            std::path::Path::new(workspace)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(workspace)
                .to_string()
        })
}

#[derive(Debug, Clone)]
pub struct ActivityManager;

impl ActivityManager {
    pub fn build_activity_fields(
        doc: Option<&Document>,
        config: &Configuration,
        workspace: &str,
        git_branch: Option<String>,
    ) -> ActivityFields {
        let workspace = resolve_workspace(workspace);
        let placeholders = Placeholders::new(doc, config, &workspace, git_branch);
        let activity = if let Some(doc) = doc {
            let language = get_language(doc).to_lowercase();
            config.languages.get(&language).unwrap_or(&config.activity)
        } else {
            &config.activity
        };
        ActivityFields::from(activity).resolve_placeholders(&placeholders)
    }

    pub fn build_idle_activity_fields(
        doc: Option<&Document>,
        config: &Configuration,
        workspace: &str,
        git_branch: Option<String>,
    ) -> ActivityFields {
        let workspace = resolve_workspace(workspace);
        let placeholders = Placeholders::new(doc, config, &workspace, git_branch);
        ActivityFields::from(&config.idle.activity).resolve_placeholders(&placeholders)
    }
}
