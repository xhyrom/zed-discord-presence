use lazy_static::lazy_static;
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

pub fn get_language(document: &Document) -> Option<String> {
    let map = LANGUAGE_MAP.lock().unwrap();

    if let Some(s) = map.get(&document.get_filename().to_string()) {
        return Some(s.to_string());
    }

    if let Some(s) = map.get(&format!(".{}", document.get_extension())) {
        return Some(s.to_string());
    }

    None
}
