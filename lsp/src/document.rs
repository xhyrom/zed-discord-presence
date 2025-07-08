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
}

impl Document {
    pub fn new(url: Url) -> Self {
        let url_path = url.path();
        let path = Path::new(url_path);

        Self {
            path: path.to_owned(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_creation() {
        let url = Url::parse("file:///home/user/test.rs").unwrap();
        let doc = Document::new(url);

        assert_eq!(doc.get_filename().unwrap(), "test.rs");
        assert_eq!(doc.get_extension(), "rs");
    }

    #[test]
    fn test_document_with_encoded_filename() {
        let url = Url::parse("file:///home/user/test%20file.rs").unwrap();
        let doc = Document::new(url);

        assert_eq!(doc.get_filename().unwrap(), "test file.rs");
    }
}
