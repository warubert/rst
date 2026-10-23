use super::SupportedLanguage;
use std::collections::HashSet;

#[test]
fn contains_all_249_languages_with_unique_codes() {
    let codes: HashSet<_> = SupportedLanguage::ALL
        .iter()
        .map(|language| language.code())
        .collect();

    assert_eq!(SupportedLanguage::ALL.len(), 249);
    assert_eq!(codes.len(), SupportedLanguage::ALL.len());
    assert_eq!(SupportedLanguage::LangEn.name(), "English");
    assert_eq!(SupportedLanguage::LangEn.name_for("pt"), "Inglês");
    assert_eq!(SupportedLanguage::LangEn.name_for("es"), "inglés");
    assert_eq!(SupportedLanguage::LangPt.code(), "pt");
    assert_eq!(SupportedLanguage::LangPtPt.code(), "pt-PT");
    assert_eq!(SupportedLanguage::LangZhCn.code(), "zh-CN");
}
