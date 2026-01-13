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

use crate::error::{PresenceError, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone)]
pub struct Document {
    path: PathBuf,
    workspace_root: PathBuf,
}

impl Document {
    pub fn new(url: &Url, workspace_root: &Path) -> Self {
        let url_path = url.path();
        let path = Path::new(url_path);

        Self {
            path: path.to_owned(),
            workspace_root: workspace_root.to_owned(),
        }
    }

    pub fn get_filename(&self) -> Result<String> {
        let filename = self
            .path
            .file_name()
            .ok_or_else(|| PresenceError::Config("No filename found".to_string()))?
            .to_str()
            .ok_or_else(|| PresenceError::Config("Invalid filename encoding".to_string()))?;

        let decoded_filename = urlencoding::decode(filename)?;
        Ok(decoded_filename.to_string())
    }

    pub fn get_extension(&self) -> &str {
        self.path
            .extension()
            .unwrap_or(OsStr::new(""))
            .to_str()
            .unwrap_or("")
    }

    pub fn get_relative_file_path(&self) -> Result<String> {
        let relative_path = self.path.strip_prefix(&self.workspace_root).map_err(|_| {
            PresenceError::Config("File is not within the workspace root".to_string())
        })?;

        Ok(relative_path.to_str().unwrap_or("").to_string())
    }

    pub fn get_full_directory_name(&self) -> Result<String> {
        let parent_dir = self.path.parent().ok_or_else(|| {
            PresenceError::Config("Could not determine parent directory".to_string())
        })?;

        Ok(parent_dir.to_str().unwrap_or("").to_string())
    }

    pub fn get_directory_name(&self) -> Result<String> {
        let parent_dir = self.path.parent().ok_or_else(|| {
            PresenceError::Config("Could not determine parent directory".to_string())
        })?;

        let dir_name = parent_dir.file_name().ok_or_else(|| {
            PresenceError::Config("Could not determine directory name".to_string())
        })?;

        Ok(dir_name.to_str().unwrap_or("").to_string())
    }

    pub fn get_folder_and_file(&self) -> Result<String> {
        let parent = self.get_directory_name()?;
        let file = self.get_filename()?;

        Ok(Path::new(&parent)
            .join(file)
            .to_str()
            .unwrap_or("")
            .to_string())
    }

    /// Gets the file size in bytes.
    pub fn get_file_size(&self) -> Result<u64> {
        std::fs::metadata(&self.path)
            .map(|m| m.len())
            .map_err(|e| PresenceError::Config(format!("Failed to get file size: {e}")))
    }

    /// Gets the file size in a human-readable format (e.g., "1.2 KB").
    pub fn get_formatted_file_size(&self) -> String {
        match self.get_file_size() {
            Ok(size) => format_file_size(size),
            Err(_) => "unknown".to_string(),
        }
    }
}

/// Formats a file size in bytes to a human-readable string.
#[allow(clippy::cast_precision_loss)]
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_creation() {
        let url = Url::parse("file:///home/user/project/test.rs").unwrap();
        let workspace_root = Path::new("/home/user/project");
        let doc = Document::new(&url, workspace_root);

        assert_eq!(doc.get_filename().unwrap(), "test.rs");
        assert_eq!(doc.get_extension(), "rs");
        assert_eq!(doc.get_relative_file_path().unwrap(), "test.rs");
        assert_eq!(doc.get_full_directory_name().unwrap(), "/home/user/project");
        assert_eq!(doc.get_directory_name().unwrap(), "project");
        assert_eq!(doc.get_folder_and_file().unwrap(), "project/test.rs");
    }

    #[test]
    fn test_document_with_encoded_filename() {
        let url = Url::parse("file:///home/user/project/test%20file.rs").unwrap();
        let workspace_root = Path::new("/home/user/project");
        let doc = Document::new(&url, workspace_root);

        assert_eq!(doc.get_filename().unwrap(), "test file.rs");
    }
}
