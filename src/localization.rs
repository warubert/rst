use std::{fs, io, path::Path};

pub fn initialize() {
    let saved_language = preference_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|locale| locale.trim().to_string());
    let system_language = sys_locale::get_locale();
    let language = select_language(saved_language.as_deref(), system_language.as_deref());

    rust_i18n::set_locale(language);
}

pub fn set_language(language: &str) {
    let language = supported_language(language).unwrap_or("en");
    rust_i18n::set_locale(language);

    if let Some(path) = preference_path() {
        let _ = persist_language(&path, language);
    }
}

fn select_language(saved: Option<&str>, system: Option<&str>) -> &'static str {
    saved
        .and_then(supported_language)
        .or_else(|| system.and_then(supported_language))
        .unwrap_or("en")
}

fn supported_language(locale: &str) -> Option<&'static str> {
    match locale
        .trim()
        .split(['-', '_'])
        .next()?
        .to_ascii_lowercase()
        .as_str()
    {
        "pt" => Some("pt"),
        "en" => Some("en"),
        "es" => Some("es"),
        _ => None,
    }
}

fn persist_language(path: &Path, language: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, language)
}

fn preference_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|path| path.join("rst").join("language"))
}

#[cfg(test)]
#[path = "tests/localization.rs"]
mod tests;
