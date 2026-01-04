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
    relative_file_path: Option<String>,
    folder_and_file: Option<String>,
    directory_name: Option<String>,
    full_directory_name: Option<String>,
    line_number: Option<u32>,
}

impl<'a> Placeholders<'a> {
    pub fn new(doc: Option<&'a Document>, config: &'a Configuration, workspace: &'a str) -> Self {
        let (
            filename,
            language,
            relative_file_path,
            folder_and_file,
            directory_name,
            full_directory_name,
            line_number,
        ) = if let Some(doc) = doc {
            (
                Some(doc.get_filename().unwrap_or_default()),
                Some(get_language(doc)),
                Some(doc.get_relative_file_path().unwrap_or_default()),
                Some(doc.get_folder_and_file().unwrap_or_default()),
                Some(doc.get_directory_name().unwrap_or_default()),
                Some(doc.get_full_directory_name().unwrap_or_default()),
                doc.get_line_number(),
            )
        } else {
            (None, None, None, None, None, None, None)
        };

        Self {
            filename,
            workspace,
            language,
            base_icons_url: &config.base_icons_url,
            relative_file_path,
            folder_and_file,
            directory_name,
            full_directory_name,
            line_number,
        }
    }

    pub fn replace(&self, text: &str) -> String {
        let filename = self.filename.as_deref().unwrap_or("filename");
        let language = self.language.as_deref().unwrap_or("language");
        let relative_file_path = self
            .relative_file_path
            .as_deref()
            .unwrap_or("relative_file_path");
        let folder_and_file = self.folder_and_file.as_deref().unwrap_or("folder_and_file");
        let directory_name = self.directory_name.as_deref().unwrap_or("directory_name");
        let full_directory_name = self
            .full_directory_name
            .as_deref()
            .unwrap_or("full_directory_name");
        
        // Convert 0-indexed line number to 1-indexed for display
        let line_number_str = self
            .line_number
            .map_or_else(|| "0".to_string(), |n| (n + 1).to_string());

        let mut result = replace_with_capitalization!(
            text,
            "filename" => filename,
            "workspace" => self.workspace,
            "language" => language,
            "base_icons_url" => self.base_icons_url,
            "relative_file_path" => relative_file_path,
            "folder_and_file" => folder_and_file,
            "directory_name" => directory_name,
            "full_directory_name" => full_directory_name
        );
        
        // Replace line_number placeholder (no capitalization variants needed for numbers)
        result = result.replace("{line_number}", &line_number_str);
        
        result
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
            relative_file_path: Some("src/test.rs".to_string()),
            folder_and_file: Some("src/test.rs".to_string()),
            directory_name: Some("src".to_string()),
            full_directory_name: Some("my-project/src".to_string()),
            line_number: Some(41), // 0-indexed, so will display as 42
        };

        let result = placeholders.replace("Working on {filename} in {workspace}");
        assert_eq!(result, "Working on test.rs in my-project");

        let result = placeholders.replace("{language:u} file");
        assert_eq!(result, "Rust file");
        
        let result = placeholders.replace("Line {line_number}");
        assert_eq!(result, "Line 42");
    }
}
