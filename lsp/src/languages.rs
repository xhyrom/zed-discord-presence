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

use lazy_static::lazy_static;
use regex::RegexBuilder;
use serde_json::from_str;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::Document;

lazy_static! {
    static ref LANGUAGE_MAP: Mutex<HashMap<String, String>> = {
        let data = include_str!("../../assets/languages.json");
        let data: HashMap<String, String> = from_str(data).unwrap();
        Mutex::new(data)
    };
}

pub fn get_language(document: &Document) -> String {
    let map = LANGUAGE_MAP.lock().unwrap();

    let filename = document
        .get_filename()
        .unwrap_or_else(|_| "unknown".to_string());
    let extension = format!(".{}", document.get_extension());

    if let Some(s) = map.get(&filename) {
        return s.to_string();
    }

    for (pattern, language) in map.iter() {
        let pattern = pattern.strip_prefix("regex:");
        if pattern.is_none() {
            continue;
        }

        if let Ok(re) = RegexBuilder::new(pattern.unwrap())
            .case_insensitive(true)
            .build()
        {
            if re.is_match(&filename) || re.is_match(&extension) {
                return language.to_string();
            }
        }
    }

    if let Some(s) = map.get(&extension) {
        return s.to_string();
    }

    String::from("text")
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::Url;

    use super::*;

    #[test]
    fn test_unicode_perl() {
        let document = Document::new(Url::parse("file:///home/user/file.php").unwrap());
        let lang = get_language(&document);
        assert_eq!(lang, "php");
    }
}
