use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
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
    let filename = document.get_filename().to_string();
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
