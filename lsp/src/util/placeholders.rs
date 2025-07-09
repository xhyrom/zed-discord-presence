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

use crate::{config::Configuration, languages::get_language, Document};

macro_rules! replace_with_capitalization {
    ($text:expr, $($placeholder:expr => $value:expr),*) => {{
        let mut result = $text.to_string();
        $(
            let capitalized = super::capitalize_first_letter($value);
            let lowercase = $value.to_lowercase();

            result = result.replace(concat!("{", $placeholder, "}"), $value)
                           .replace(concat!("{", $placeholder, ":u}"), &capitalized)
                           .replace(concat!("{", $placeholder, ":lo}"), &lowercase);
        )*
        result
    }};
}

pub struct Placeholders<'a> {
    filename: Option<String>,
    workspace: &'a str,
    language: Option<String>,
    base_icons_url: &'a str,
}

impl<'a> Placeholders<'a> {
    pub fn new(doc: Option<&'a Document>, config: &'a Configuration, workspace: &'a str) -> Self {
        let (filename, language) = if let Some(doc) = doc {
            let filename = doc.get_filename().unwrap_or_else(|_| "unknown".to_string());
            (Some(filename), Some(get_language(doc)))
        } else {
            (None, None)
        };

        Self {
            filename,
            workspace,
            language,
            base_icons_url: &config.base_icons_url,
        }
    }

    pub fn replace(&self, text: &str) -> String {
        let filename = self.filename.as_deref().unwrap_or("filename");
        let language = self.language.as_deref().unwrap_or("language");

        replace_with_capitalization!(
            text,
            "filename" => filename,
            "workspace" => self.workspace,
            "language" => language,
            "base_icons_url" => self.base_icons_url
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholders_replace() {
        let placeholders = Placeholders {
            filename: Some("test.rs".to_string()),
            workspace: "my-project",
            language: Some("rust".to_string()),
            base_icons_url: "https://example.com",
        };

        let result = placeholders.replace("Working on {filename} in {workspace}");
        assert_eq!(result, "Working on test.rs in my-project");

        let result = placeholders.replace("{language:u} file");
        assert_eq!(result, "Rust file");
    }
}
