use crate::interfaces::translate::Translator;
use crate::srt;
use rust_i18n::t;
use std::fs;

mod cues;
mod providers;
mod text;

use cues::RETRY_DELAY;
pub use cues::translate_cues_with_progress;
pub use providers::{GoogleTranslator, GtxTranslator};

pub use crate::settings::API_KEY_ENV;

pub fn api_key() -> Option<String> {
    crate::settings::translation_api_key()
}

pub fn source_language_code(language: &str) -> String {
    let normalized = language.trim().replace('_', "-").to_ascii_lowercase();
    let unknown = t!("tui.unknown_language").to_string().to_ascii_lowercase();
    if normalized.is_empty() || normalized == "und" || normalized == unknown {
        return "auto".to_string();
    }

    let mut parts = normalized.splitn(2, '-');
    let base = parts.next().unwrap_or_default();
    let language = match base {
        "eng" => "en",
        "por" => "pt",
        "spa" => "es",
        "fra" | "fre" => "fr",
        "deu" | "ger" => "de",
        "ita" => "it",
        "nld" | "dut" => "nl",
        "jpn" => "ja",
        "kor" => "ko",
        "rus" => "ru",
        "zho" | "chi" => "zh",
        _ => base,
    };

    match parts.next() {
        Some(region) => format!("{language}-{region}"),
        None => language.to_string(),
    }
}

pub fn translate_file(
    input: &str,
    output: &str,
    source: &str,
    target: &str,
    api_key: Option<&str>,
) -> Result<(), String> {
    match api_key {
        Some(key) => translate_file_with(
            input,
            output,
            source,
            target,
            &GoogleTranslator::new(key.to_string()),
        ),
        None => translate_file_with(input, output, source, target, &GtxTranslator::new()),
    }
}

pub fn translate_file_with_progress(
    input: &str,
    output: &str,
    source: &str,
    target: &str,
    api_key: Option<&str>,
    on_progress: &mut dyn FnMut(u8),
) -> Result<(), String> {
    match api_key {
        Some(key) => translate_file_with_progress_using(
            input,
            output,
            source,
            target,
            &GoogleTranslator::new(key.to_string()),
            on_progress,
        ),
        None => translate_file_with_progress_using(
            input,
            output,
            source,
            target,
            &GtxTranslator::new(),
            on_progress,
        ),
    }
}

pub fn translate_file_with<T: Translator>(
    input: &str,
    output: &str,
    source: &str,
    target: &str,
    translator: &T,
) -> Result<(), String> {
    translate_file_with_progress_using(input, output, source, target, translator, &mut |_| {})
}

fn translate_file_with_progress_using<T: Translator>(
    input: &str,
    output: &str,
    source: &str,
    target: &str,
    translator: &T,
    on_progress: &mut dyn FnMut(u8),
) -> Result<(), String> {
    let content = fs::read_to_string(input)
        .map_err(|error| t!("errors.srt_read", path = input, error = error).to_string())?;
    let cues = srt::parse(&content)?;

    on_progress(0);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| t!("errors.translate_request", error = error).to_string())?;
    let translated = runtime.block_on(translate_cues_with_progress(
        translator,
        &cues,
        source,
        target,
        RETRY_DELAY,
        on_progress,
    ))?;

    fs::write(output, srt::serialize(&translated))
        .map_err(|error| t!("errors.srt_write", path = output, error = error).to_string())?;
    on_progress(100);
    Ok(())
}

#[cfg(test)]
#[path = "../tests/translate.rs"]
mod tests;
