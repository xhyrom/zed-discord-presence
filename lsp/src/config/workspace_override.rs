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

/// Three-state encoding for per-workspace activity field overrides.
///
/// - `Inherit` — not set; use the language / global template value.
/// - `Clear`   — explicitly set to `null`; removes the field from presence.
/// - `Set(s)`  — override with the given string (may contain placeholders).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum OverrideField {
    #[default]
    Inherit,
    Clear,
    Set(String),
}

impl OverrideField {
    /// Parses the three-state value from a JSON object key.
    pub fn from_json(json: &Value, key: &str) -> Self {
        match json.get(key) {
            Some(Value::Null) => Self::Clear,
            Some(Value::String(s)) => Self::Set(s.clone()),
            _ => Self::Inherit,
        }
    }
}

/// Workspace-level match criteria. All non-empty conditions must hold (AND).
/// An empty list for a condition means "match anything" for that condition.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceOverrideMatch {
    /// Workspace path prefixes to match (e.g. `"/home/user/work"`).
    pub paths: Vec<String>,
    /// Git repository names to match (last URL segment, without `.git`).
    pub repo_names: Vec<String>,
}

/// Convenience flags for hiding individual details from the presence display.
/// Using `#[allow]` here because all four fields are semantically distinct
/// boolean flags with no richer state to encode.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct HideFields {
    /// Hide the active filename from all templates.
    pub filename: bool,
    /// Disable git integration (hides repository name / link).
    pub repo_name: bool,
    /// Hide all file-path placeholders (relative path, folder, directory).
    pub file_path: bool,
    /// Hide the current line number.
    pub line_number: bool,
}

/// Per-workspace presence override. Fields that are `Inherit` fall back to
/// the language or global template value.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceOverride {
    pub match_: WorkspaceOverrideMatch,
    pub state: OverrideField,
    pub details: OverrideField,
    pub large_image: OverrideField,
    pub large_text: OverrideField,
    pub small_image: OverrideField,
    pub small_text: OverrideField,
    pub git_integration: Option<bool>,
    pub hide: Option<HideFields>,
}

impl WorkspaceOverride {
    /// Returns `true` when this override applies to the given workspace.
    pub fn matches(&self, workspace_path: &str, git_remote_url: Option<&str>) -> bool {
        let path_match = self.match_.paths.is_empty()
            || self
                .match_
                .paths
                .iter()
                .any(|p| workspace_path.starts_with(p.as_str()));

        let repo_match = self.match_.repo_names.is_empty()
            || git_remote_url.is_some_and(|url| {
                let repo = extract_repo_name(url);
                self.match_.repo_names.iter().any(|n| n == repo)
            });

        path_match && repo_match
    }

    /// Resolves whether git integration should be enabled for this override,
    /// falling back to `default` when neither `git_integration` nor
    /// `hide.repo_name` is set.
    pub fn effective_git_integration(&self, default: bool) -> bool {
        if let Some(gi) = self.git_integration {
            return gi;
        }
        if self.hide.as_ref().is_some_and(|h| h.repo_name) {
            return false;
        }
        default
    }
}

/// Extracts the repository name from a remote URL by stripping the `.git`
/// suffix and returning the last path segment.
fn extract_repo_name(url: &str) -> &str {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or(url)
}

fn parse_hide(json: &Value) -> Option<HideFields> {
    let hide = json.get("hide")?;
    let flag = |k| hide.get(k).and_then(Value::as_bool).unwrap_or(false);

    Some(HideFields {
        filename: flag("filename"),
        repo_name: flag("repo_name"),
        file_path: flag("file_path"),
        line_number: flag("line_number"),
    })
}

fn parse_one(json: &Value) -> WorkspaceOverride {
    let mut ov = WorkspaceOverride::default();

    if let Some(m) = json.get("match") {
        if let Some(paths) = m.get("paths").and_then(Value::as_array) {
            ov.match_.paths = paths
                .iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect();
        }
        if let Some(repos) = m.get("repo_names").and_then(Value::as_array) {
            ov.match_.repo_names = repos
                .iter()
                .filter_map(|r| r.as_str().map(String::from))
                .collect();
        }
    }

    ov.state = OverrideField::from_json(json, "state");
    ov.details = OverrideField::from_json(json, "details");
    ov.large_image = OverrideField::from_json(json, "large_image");
    ov.large_text = OverrideField::from_json(json, "large_text");
    ov.small_image = OverrideField::from_json(json, "small_image");
    ov.small_text = OverrideField::from_json(json, "small_text");
    ov.git_integration = json.get("git_integration").and_then(Value::as_bool);
    ov.hide = parse_hide(json);

    ov
}

pub fn parse_workspace_overrides(json: &Value) -> Vec<WorkspaceOverride> {
    json.get("overrides")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(parse_one).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_by_path_prefix() {
        let ov = parse_one(&serde_json::json!({
            "match": { "paths": ["/home/user/work"] }
        }));
        assert!(ov.matches("/home/user/work/repo", None));
        assert!(ov.matches("/home/user/work", None));
        assert!(!ov.matches("/home/user/personal", None));
    }

    #[test]
    fn test_matches_by_repo_name() {
        let ov = parse_one(&serde_json::json!({
            "match": { "repo_names": ["secret-api"] }
        }));
        assert!(ov.matches("", Some("git@github.com:org/secret-api.git")));
        assert!(ov.matches("", Some("https://github.com/org/secret-api")));
        assert!(!ov.matches("", Some("https://github.com/org/public-api")));
    }

    #[test]
    fn test_matches_combined_and() {
        let ov = parse_one(&serde_json::json!({
            "match": {
                "paths": ["/work"],
                "repo_names": ["secret"]
            }
        }));
        // Path under /work AND repo name matches → match
        assert!(ov.matches("/work/secret", Some("https://github.com/org/secret")));
        // Path outside /work → no match even if repo matches
        assert!(!ov.matches("/personal/secret", Some("https://github.com/org/secret")));
        // Path under /work but repo name differs → no match
        assert!(!ov.matches("/work/secret", Some("https://github.com/org/other")));
    }

    #[test]
    fn test_empty_match_always_matches() {
        let ov = WorkspaceOverride::default();
        assert!(ov.matches("/any/path", None));
        assert!(ov.matches("", Some("https://github.com/org/repo")));
    }

    #[test]
    fn test_override_fields() {
        let ov = parse_one(&serde_json::json!({
            "match": {},
            "state": "Working in Zed",
            "details": null
        }));
        assert_eq!(ov.state, OverrideField::Set("Working in Zed".to_string()));
        assert_eq!(ov.details, OverrideField::Clear);
        assert_eq!(ov.large_image, OverrideField::Inherit);
    }

    #[test]
    fn test_effective_git_integration() {
        let explicit_off = parse_one(&serde_json::json!({
            "match": {}, "git_integration": false
        }));
        assert!(!explicit_off.effective_git_integration(true));

        let hide_repo = parse_one(&serde_json::json!({
            "match": {}, "hide": { "repo_name": true }
        }));
        assert!(!hide_repo.effective_git_integration(true));

        let inherit = parse_one(&serde_json::json!({ "match": {} }));
        assert!(inherit.effective_git_integration(true));
        assert!(!inherit.effective_git_integration(false));
    }

    #[test]
    fn test_parse_workspace_overrides() {
        let json = serde_json::json!({
            "overrides": [
                {
                    "match": { "paths": ["/work"] },
                    "state": "Work mode",
                    "git_integration": false
                },
                {
                    "match": { "repo_names": ["private"] },
                    "details": null,
                    "hide": { "filename": true, "file_path": true }
                }
            ]
        });

        let overrides = parse_workspace_overrides(&json);
        assert_eq!(overrides.len(), 2);

        let first = &overrides[0];
        assert_eq!(first.match_.paths, vec!["/work"]);
        assert_eq!(first.state, OverrideField::Set("Work mode".to_string()));
        assert_eq!(first.git_integration, Some(false));

        let second = &overrides[1];
        assert_eq!(second.match_.repo_names, vec!["private"]);
        assert_eq!(second.details, OverrideField::Clear);
        let hide = second.hide.as_ref().unwrap();
        assert!(hide.filename);
        assert!(hide.file_path);
        assert!(!hide.line_number);
    }
}
