use std::fs;

pub fn initialize() {
    let saved_language = preference_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|locale| supported_language(&locale));
    let system_language = sys_locale::get_locale().and_then(|locale| supported_language(&locale));
    let language = saved_language.or(system_language).unwrap_or("en");

    rust_i18n::set_locale(language);
}

pub fn set_language(language: &str) {
    let language = supported_language(language).unwrap_or("en");
    rust_i18n::set_locale(language);

    if let Some(path) = preference_path()
        && let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_ok()
    {
        let _ = fs::write(path, language);
    }
}

fn supported_language(locale: &str) -> Option<&'static str> {
    match locale
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

fn preference_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|path| path.join("rst").join("language"))
}

#[cfg(test)]
mod tests {
    use super::supported_language;

    #[test]
    fn maps_supported_locale_tags() {
        assert_eq!(supported_language("pt_BR"), Some("pt"));
        assert_eq!(supported_language("en-US"), Some("en"));
        assert_eq!(supported_language("es_ES"), Some("es"));
        assert_eq!(supported_language("fr-FR"), None);
    }
}
