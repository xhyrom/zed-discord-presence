use serde_json::Value;

use crate::config::update::{update_optional_string_field, UpdateFromJson};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Activity {
    pub state: Option<String>,
    pub details: Option<String>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            state: Some(String::from("Working on {filename}")),
            details: Some(String::from("In {workspace}")),
            large_image: Some(String::from("{base_icons_url}/{language:lo}.png")),
            large_text: Some(String::from("{language:u}")),
            small_image: Some(String::from("{base_icons_url}/zed.png")),
            small_text: Some(String::from("Zed")),
        }
    }
}

impl UpdateFromJson for Activity {
    fn update_from_json(&mut self, json: &Value) -> Result<()> {
        update_optional_string_field!(self, json, state, "state");
        update_optional_string_field!(self, json, details, "details");
        update_optional_string_field!(self, json, large_image, "large_image");
        update_optional_string_field!(self, json, large_text, "large_text");
        update_optional_string_field!(self, json, small_image, "small_image");
        update_optional_string_field!(self, json, small_text, "small_text");

        Ok(())
    }
}
