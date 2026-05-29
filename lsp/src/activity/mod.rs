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
    config::{
        workspace_override::{OverrideField, WorkspaceOverride},
        Configuration,
    },
    document::Document,
    languages::get_language,
    util::Placeholders,
};

#[derive(Debug, Clone)]
pub struct ActivityManager;

impl ActivityManager {
    pub fn build_activity_fields(
        doc: Option<&Document>,
        config: &Configuration,
        workspace: &str,
        workspace_path: &str,
        git_branch: Option<String>,
        git_remote_url: Option<&str>,
    ) -> ActivityFields {
        let workspace_override = config.find_workspace_override(workspace_path, git_remote_url);

        let mut placeholders = Placeholders::new(doc, config, workspace, git_branch);
        if let Some(ov) = workspace_override {
            if let Some(ref hide) = ov.hide {
                placeholders = placeholders.with_hidden_fields(hide);
            }
        }

        // Precedence: workspace override > language override > global defaults
        let activity = if let Some(doc) = doc {
            let language = get_language(doc).to_lowercase();
            config.languages.get(&language).unwrap_or(&config.activity)
        } else {
            &config.activity
        };

        let mut fields = ActivityFields::from(activity);
        if let Some(ov) = workspace_override {
            apply_override_fields(&mut fields, ov);
        }

        fields.resolve_placeholders(&placeholders)
    }

    pub fn build_idle_activity_fields(
        doc: Option<&Document>,
        config: &Configuration,
        workspace: &str,
        git_branch: Option<String>,
    ) -> ActivityFields {
        let placeholders = Placeholders::new(doc, config, workspace, git_branch);

        ActivityFields::from(&config.idle.activity).resolve_placeholders(&placeholders)
    }
}

/// Applies `WorkspaceOverride` fields onto `ActivityFields`.
/// `OverrideField::Set(s)` sets the field; `Clear` removes it; `Inherit` is a no-op.
fn apply_override_fields(fields: &mut ActivityFields, ov: &WorkspaceOverride) {
    macro_rules! apply {
        ($field:ident) => {
            match &ov.$field {
                OverrideField::Set(v) => fields.$field = Some(v.clone()),
                OverrideField::Clear => fields.$field = None,
                OverrideField::Inherit => {}
            }
        };
    }
    apply!(state);
    apply!(details);
    apply!(large_image);
    apply!(large_text);
    apply!(small_image);
    apply!(small_text);
}
