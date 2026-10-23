use crate::interfaces::probe::{Subtitle, SubtitleProbe};
use rust_i18n::t;

#[derive(Debug, PartialEq, Eq)]
pub enum CombinedOutcome {
    NoSubtitles,
    CancelledBeforeExtraction,
    ExtractedAndTranslated {
        extracted: String,
        translated: String,
    },
    TranslationCancelled {
        extracted: String,
    },
    TranslationFailed {
        extracted: String,
        error: String,
    },
}

pub fn extract_and_translate<P, S, O, E, T>(
    input: &str,
    probe: &P,
    select_subtitle: S,
    output_path: O,
    extract: E,
    translate: T,
) -> Result<CombinedOutcome, String>
where
    P: SubtitleProbe,
    S: FnOnce(&[Subtitle]) -> Option<usize>,
    O: FnOnce(&str) -> Option<String>,
    E: FnOnce(&str, i64, &str) -> Result<(), String>,
    T: FnOnce(&str, &str) -> Result<Option<String>, String>,
{
    let subtitle_streams = probe.get_subtitle_streams(input)?;
    if subtitle_streams.is_empty() {
        return Ok(CombinedOutcome::NoSubtitles);
    }

    let Some(selected_index) = select_subtitle(&subtitle_streams) else {
        return Ok(CombinedOutcome::CancelledBeforeExtraction);
    };
    let Some(selected_subtitle) = subtitle_streams.get(selected_index) else {
        return Err(t!("errors.subtitle_selection_invalid").to_string());
    };
    let Some(extracted) = output_path(input) else {
        return Ok(CombinedOutcome::CancelledBeforeExtraction);
    };

    extract(input, selected_subtitle.index, &extracted)?;

    match translate(&extracted, &selected_subtitle.language) {
        Ok(Some(translated)) => Ok(CombinedOutcome::ExtractedAndTranslated {
            extracted,
            translated,
        }),
        Ok(None) => Ok(CombinedOutcome::TranslationCancelled { extracted }),
        Err(error) => Ok(CombinedOutcome::TranslationFailed { extracted, error }),
    }
}

#[cfg(test)]
#[path = "tests/workflow.rs"]
mod tests;
