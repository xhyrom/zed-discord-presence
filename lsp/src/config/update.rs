use serde_json::Value;

pub trait UpdateFromJson {
    fn update_from_json(&mut self, json: &Value) -> Result<(), super::ConfigError>;
}

macro_rules! update_optional_string_field {
    ($target:expr, $json:expr, $field:ident, $key:expr) => {
        if let Some(value) = $json.get($key) {
            $target.$field = if value.is_null() {
                None
            } else {
                value.as_str().map(String::from)
            };
        }
    };
}

pub(crate) use update_optional_string_field;
