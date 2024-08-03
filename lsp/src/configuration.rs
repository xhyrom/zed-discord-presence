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

#[derive(Debug)]
pub struct Configuration {
    pub state: String,
    pub details: String,
    pub git_integration: bool,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            state: String::from("Working on {filename}"),
            details: String::from("In {workspace}"),
            git_integration: true,
        }
    }

    pub fn set(&mut self, initialization_options: Option<Value>) {
        if initialization_options.is_none() {
            return;
        }

        let initialization_options = initialization_options.unwrap();

        if let Some(state) = initialization_options.get("state") {
            self.state = state.as_str().unwrap().to_string();
        }

        if let Some(details) = initialization_options.get("details") {
            self.details = details.as_str().unwrap().to_string();
        }

        if let Some(git_integration) = initialization_options.get("git_integration") {
            self.git_integration = git_integration.as_bool().unwrap_or(true);
        }
    }
}
