use crate::{configuration::Configuration, languages::get_language, Document};

macro_rules! replace_with_capitalization {
    ($text:expr, $($placeholder:expr => $value:expr),*) => {{
        let mut result = $text.to_string();
        $(
            let capitalized = capitalize_first_letter($value);
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
}

impl<'a> Placeholders<'a> {
    pub fn new(doc: Option<&'a Document>, config: &'a Configuration, workspace: &'a str) -> Self {
        let (filename, language) = if let Some(doc) = doc {
            (Some(doc.get_filename()), Some(get_language(doc)))
        } else {
            (None, None)
        };

        Self {
            filename,
            workspace,
            language,
            base_icons_url: &config.base_icons_url,
        }
    }

    pub fn replace(&self, text: &str) -> String {
        let filename = self.filename.as_deref().unwrap_or("filename");
        let language = self.language.as_deref().unwrap_or("language");

        replace_with_capitalization!(
            text,
            "filename" => filename,
            "workspace" => self.workspace,
            "language" => language,
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
