use crate::{configuration::Configuration, languages::get_language, Document};

macro_rules! replace_with_capitalization {
    ($text:expr, $($placeholder:expr => $value:expr),*) => {{
        let mut result = $text.to_string();
        $(
            let capitalized = capitalize_first_letter($value);
            result = result.replace(concat!("{", $placeholder, "}"), $value)
                           .replace(concat!("{", $placeholder, ":u}"), &capitalized);
        )*
        result
    }};
}

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
        replace_with_capitalization!(
            text,
            "filename" => self.filename,
            "workspace" => self.workspace,
            "language" => self.language.as_str(),
            "base_icons_url" => self.base_icons_url
        )
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

fn capitalize_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
