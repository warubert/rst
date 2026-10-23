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
