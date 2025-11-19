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

fn get_repository(path: &str) -> Option<Repository> {
    Repository::open(path).ok()
}

fn get_main_remote_url(repository: &Repository) -> Option<String> {
    if let Ok(remote) = repository.find_remote("origin") {
        return remote.url().map(|url| transform_url(url.to_string()));
    }

    match repository.remotes() {
        Ok(remotes) => remotes.get(0).and_then(|name| {
            repository
                .find_remote(name)
                .ok()
                .and_then(|remote| remote.url().map(|url| transform_url(url.to_string())))
        }),
        Err(_) => None,
    }
}

fn transform_url(url: String) -> String {
    if url.starts_with("https://") {
        return url;
    }

    if let Some((_, rest)) = url.split_once('@') {
        if let Some((domain, path)) = rest.split_once(':') {
            return format!("https://{domain}/{path}");
        }
    }

    url.clone()
}

pub fn get_repository_and_remote(path: &str) -> Option<String> {
    match get_repository(path) {
        Some(repository) => get_main_remote_url(&repository),
        None => None,
    }
}
