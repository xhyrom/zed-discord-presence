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

use git2::Repository;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

fn get_repository(path: &str) -> Option<Repository> {
    Repository::open(path).ok()
}

fn get_main_remote_url(
    repository: &Repository,
    overrides: &HashMap<String, String>,
) -> Option<String> {
    if let Ok(remote) = repository.find_remote("origin") {
        return remote
            .url()
            .map(|url| transform_url(url.to_string(), overrides));
    }

    match repository.remotes() {
        Ok(remotes) => remotes.get(0).and_then(|name| {
            repository.find_remote(name).ok().and_then(|remote| {
                remote
                    .url()
                    .map(|url| transform_url(url.to_string(), overrides))
            })
        }),
        Err(_) => None,
    }
}

fn transform_url(mut url: String, overrides: &HashMap<String, String>) -> String {
    for (from, to) in overrides {
        url = url.replace(&format!("://{from}/"), &format!("://{to}/"));
        url = url.replace(&format!("://{from}:"), &format!("://{to}:"));
        if url.ends_with(&format!("://{from}")) {
            url = url.replace(&format!("://{from}"), &format!("://{to}"));
        }

        url = url.replace(&format!("@{from}:"), &format!("@{to}:"));
        url = url.replace(&format!("@{from}/"), &format!("@{to}/"));

        if url.starts_with(&format!("{from}:")) {
            url = url.replacen(&format!("{from}:"), &format!("{to}:"), 1);
        }
    }

    if url.starts_with("https://") || url.starts_with("http://") {
        return url;
    }

    if url.starts_with("ssh://") {
        url = url.replace("ssh://", "");
    } else if url.starts_with("git://") {
        url = url.replace("git://", "");
    }

    if let Some((_, rest)) = url.split_once('@') {
        url = rest.to_string();
    }

    if let Some((domain, path)) = url.split_once(':') {
        if !path.starts_with("//") {
            url = format!("{domain}/{path}");
        }
    } else if let Some((domain, path)) = url.split_once('/') {
        url = format!("{domain}/{path}");
    }

    if Path::new(&url)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
    {
        url = url[..url.len() - 4].to_string();
    }

    format!("https://{url}")
}

pub fn get_current_branch(path: &str) -> Option<String> {
    let repo = get_repository(path)?;
    let head = repo.head().ok()?;

    if head.is_branch() {
        head.shorthand().map(str::to_string)
    } else {
        head.target().map(|oid| {
            let hex = oid.to_string();
            hex[..7.min(hex.len())].to_string()
        })
    }
}

pub fn get_repository_and_remote(
    path: &str,
    overrides: &HashMap<String, String>,
) -> Option<String> {
    match get_repository(path) {
        Some(repository) => get_main_remote_url(&repository, overrides),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_branch_non_git_directory() {
        let branch = get_current_branch("/tmp");
        assert!(branch.is_none());
    }

    #[test]
    fn test_get_current_branch_invalid_path() {
        let branch = get_current_branch("/this/path/does/not/exist");
        assert!(branch.is_none());
    }

    #[test]
    fn test_get_current_branch_current_repo() {
        let branch = get_current_branch(".");

        if let Some(ref b) = branch {
            assert!(!b.is_empty());
        }
    }

    #[test]
    fn test_transform_url_ssh_alias_override() {
        let mut overrides = HashMap::new();
        overrides.insert("github-b".to_string(), "github.com".to_string());
        let input = "git@github-b:user/repo.git".to_string();
        let result = super::transform_url(input, &overrides);
        assert_eq!(result, "https://github.com/user/repo");
    }

    #[test]
    fn test_transform_url_https_override() {
        let mut overrides = HashMap::new();
        overrides.insert("github-b".to_string(), "github.com".to_string());
        let input = "https://github-b/user/repo".to_string();
        let result = super::transform_url(input, &overrides);
        assert_eq!(result, "https://github.com/user/repo");
    }

    #[test]
    fn test_transform_url_no_overrides_noop_for_https() {
        let overrides: HashMap<String, String> = HashMap::new();
        let input = "https://github.com/user/repo".to_string();
        let result = super::transform_url(input.clone(), &overrides);
        assert_eq!(result, input);
    }

    #[test]
    fn test_transform_url_ssh_override() {
        let mut overrides = HashMap::new();
        overrides.insert("github-b".to_string(), "github.com".to_string());
        let input = "git@github-b:user/repo.git".to_string();
        let result = super::transform_url(input, &overrides);
        assert_eq!(result, "https://github.com/user/repo");
    }
}
