use std::ffi::OsString;

pub fn value(
    get: &impl Fn(&str) -> Option<OsString>,
    name: &str,
) -> Result<Option<String>, String> {
    let Some(value) = get(name) else {
        return Ok(None);
    };
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not Unicode"))?;
    if value.chars().any(char::is_control) {
        return Err(format!("{name} contains control characters"));
    }
    Ok((!value.is_empty()).then_some(value))
}
