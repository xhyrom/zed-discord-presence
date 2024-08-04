use crate::{configuration::Configuration, languages::get_language, Document};

pub struct Placeholders<'a> {
    filename: &'a str,
    workspace: &'a str,
    language: String,
    base_icons_url: &'a str,
}

impl<'a> Placeholders<'a> {
    pub fn new(doc: &'a Document, config: &'a Configuration, workspace: &'a str) -> Self {
        Self {
            filename: doc.get_filename(),
            workspace,
            language: get_language(doc).unwrap(),
            base_icons_url: &config.base_icons_url,
        }
    }

    pub fn replace(&self, text: &str) -> String {
        text.replace("{filename}", self.filename)
            .replace("{workspace}", self.workspace)
            .replace("{language}", self.language.as_str())
            .replace("{base_icons_url}", self.base_icons_url)
    }
}

pub fn set_optional_field<'a, T, F>(mut obj: T, field: Option<&'a str>, setter: F) -> T
where
    F: FnOnce(T, &'a str) -> T,
{
    if let Some(value) = field {
        obj = setter(obj, value);
    }
    obj
}
