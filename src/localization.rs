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
mod tests {
    use super::{persist_language, select_language, supported_language};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn maps_supported_locale_tags() {
        assert_eq!(supported_language("pt_BR"), Some("pt"));
        assert_eq!(supported_language("pt_br"), Some("pt"));
        assert_eq!(supported_language("en-US"), Some("en"));
        assert_eq!(supported_language("es_ES"), Some("es"));
        assert_eq!(supported_language("fr-FR"), None);
    }

    #[test]
    fn saved_language_takes_precedence_over_system_locale() {
        assert_eq!(select_language(Some("es\n"), Some("pt_BR")), "es");
        assert_eq!(select_language(Some("fr"), Some("pt_BR")), "pt");
        assert_eq!(select_language(None, Some("fr_FR")), "en");
    }

    #[test]
    fn persists_selected_language_to_the_given_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("rst-locale-test-{unique}"));
        let path = directory.join("settings").join("language");

        persist_language(&path, "es").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "es");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn provides_translations_for_all_supported_languages() {
        assert_eq!(
            rust_i18n::t!("menu.extract", locale = "pt").to_string(),
            "1) Extrair legenda"
        );
        assert_eq!(
            rust_i18n::t!("menu.extract", locale = "en").to_string(),
            "1) Extract subtitles"
        );
        assert_eq!(
            rust_i18n::t!("menu.extract", locale = "es").to_string(),
            "1) Extraer subtítulos"
        );
    }
}
