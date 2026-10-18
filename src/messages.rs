use crate::translate;
use rust_i18n::t;

pub fn translation_success_message(output: &str, keyless: bool) -> String {
    let mut message = t!("messages.translation_success", output = output).to_string();
    if keyless {
        message.push_str("\n\n");
        message.push_str(&t!(
            "messages.translation_without_key",
            name = translate::API_KEY_ENV
        ));
    }
    message
}

pub fn combined_success_message(extracted: &str, translated: &str, keyless: bool) -> String {
    let mut message = t!(
        "messages.extract_translate_success",
        extracted = extracted,
        translated = translated
    )
    .to_string();
    if keyless {
        message.push_str("\n\n");
        message.push_str(&t!(
            "messages.translation_without_key",
            name = translate::API_KEY_ENV
        ));
    }
    message
}

pub fn whisper_translation_success_message(
    transcribed: &str,
    translated: &str,
    keyless: bool,
) -> String {
    let mut message = t!(
        "whisper.translation_success",
        transcribed = transcribed,
        translated = translated
    )
    .to_string();
    if keyless {
        message.push_str("\n\n");
        message.push_str(&t!(
            "messages.translation_without_key",
            name = translate::API_KEY_ENV
        ));
    }
    message
}
