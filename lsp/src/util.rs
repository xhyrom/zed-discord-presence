pub fn set_optional_field<'a, T, F>(mut obj: T, field: Option<&'a str>, setter: F) -> T
where
    F: FnOnce(T, &'a str) -> T,
{
    if let Some(value) = field {
        obj = setter(obj, value);
    }
    obj
}
