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
mod tests {
    use super::{CombinedOutcome, extract_and_translate};
    use crate::{
        interfaces::{
            probe::{Subtitle, SubtitleProbe},
            translate::Translator,
        },
        srt, translate,
    };
    use std::{
        cell::RefCell,
        fs,
        rc::Rc,
        time::{SystemTime, UNIX_EPOCH},
    };

    const INPUT_SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\n{\\an8}Hello world.\n";

    struct FakeProbe {
        streams: Vec<Subtitle>,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    impl SubtitleProbe for FakeProbe {
        fn get_subtitle_streams(&self, _input: &str) -> Result<Vec<Subtitle>, String> {
            self.events.borrow_mut().push("probe");
            Ok(self
                .streams
                .iter()
                .map(|stream| Subtitle {
                    index: stream.index,
                    language: stream.language.clone(),
                    is_sdh: stream.is_sdh,
                })
                .collect())
        }
    }

    struct PrefixTranslator;

    impl Translator for PrefixTranslator {
        async fn translate_batch(
            &self,
            texts: &[String],
            _source: &str,
            _target: &str,
        ) -> Result<Vec<String>, String> {
            Ok(texts.iter().map(|text| format!("PT: {text}")).collect())
        }
    }

    fn temp_directory() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rst-combined-flow-{unique}"))
    }

    fn fake_probe(streams: Vec<Subtitle>, events: Rc<RefCell<Vec<&'static str>>>) -> FakeProbe {
        FakeProbe { streams, events }
    }

    fn subtitle_streams() -> Vec<Subtitle> {
        vec![Subtitle {
            index: 2,
            language: "en".to_string(),
            is_sdh: false,
        }]
    }

    #[test]
    fn runs_probe_extract_and_translate_in_order_and_writes_both_srt_files() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        let extracted_path = directory.join("episode.srt");
        let translated_path = directory.join("episode-pt.srt");
        let extracted_path = extracted_path.to_string_lossy().into_owned();
        let translated_path = translated_path.to_string_lossy().into_owned();
        let events = Rc::new(RefCell::new(Vec::new()));
        let probe = fake_probe(subtitle_streams(), events.clone());

        let outcome = extract_and_translate(
            "episode.mkv",
            &probe,
            |streams| {
                events.borrow_mut().push("select");
                assert_eq!(streams.len(), 1);
                Some(0)
            },
            |input| {
                events.borrow_mut().push("output");
                assert_eq!(input, "episode.mkv");
                Some(extracted_path.clone())
            },
            |input, index, output| {
                events.borrow_mut().push("extract");
                assert_eq!(input, "episode.mkv");
                assert_eq!(index, 2);
                fs::write(output, INPUT_SRT).map_err(|error| error.to_string())
            },
            |input_srt, source_language| {
                events.borrow_mut().push("translate");
                assert_eq!(input_srt, extracted_path);
                assert_eq!(source_language, "en");
                translate::translate_file_with(
                    input_srt,
                    &translated_path,
                    source_language,
                    "pt",
                    &PrefixTranslator,
                )
                .map_err(|error| error.to_string())?;
                Ok(Some(translated_path.clone()))
            },
        )
        .unwrap();

        assert_eq!(
            outcome,
            CombinedOutcome::ExtractedAndTranslated {
                extracted: extracted_path.clone(),
                translated: translated_path.clone(),
            }
        );
        assert_eq!(
            *events.borrow(),
            ["probe", "select", "output", "extract", "translate"]
        );
        assert_eq!(fs::read_to_string(&extracted_path).unwrap(), INPUT_SRT);

        let original = srt::parse(&fs::read_to_string(&extracted_path).unwrap()).unwrap();
        let translated = srt::parse(&fs::read_to_string(&translated_path).unwrap()).unwrap();
        assert_eq!(original.len(), 1);
        assert_eq!(translated.len(), 1);
        assert_eq!(original[0].index, translated[0].index);
        assert_eq!(original[0].timing, translated[0].timing);
        assert_eq!(translated[0].text, "{\\an8}PT: Hello world.");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn does_not_extract_or_translate_when_probe_finds_no_subtitles() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let probe = fake_probe(Vec::new(), events.clone());

        let outcome = extract_and_translate(
            "episode.mkv",
            &probe,
            |_| panic!("must not select a subtitle"),
            |_| panic!("must not prompt for output"),
            |_, _, _| panic!("must not extract"),
            |_, _| panic!("must not translate"),
        )
        .unwrap();

        assert_eq!(outcome, CombinedOutcome::NoSubtitles);
        assert_eq!(*events.borrow(), ["probe"]);
    }

    #[test]
    fn reports_extraction_without_translation_when_translation_is_cancelled() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        let extracted_path = directory.join("episode.srt").to_string_lossy().into_owned();
        let probe = fake_probe(subtitle_streams(), Rc::new(RefCell::new(Vec::new())));

        let outcome = extract_and_translate(
            "episode.mkv",
            &probe,
            |_| Some(0),
            |_| Some(extracted_path.clone()),
            |_, _, output| fs::write(output, INPUT_SRT).map_err(|error| error.to_string()),
            |_, _| Ok(None),
        )
        .unwrap();

        assert_eq!(
            outcome,
            CombinedOutcome::TranslationCancelled {
                extracted: extracted_path.clone()
            }
        );
        assert_eq!(fs::read_to_string(extracted_path).unwrap(), INPUT_SRT);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn preserves_extracted_srt_when_translation_fails() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        let extracted_path = directory.join("episode.srt").to_string_lossy().into_owned();
        let probe = fake_probe(subtitle_streams(), Rc::new(RefCell::new(Vec::new())));

        let outcome = extract_and_translate(
            "episode.mkv",
            &probe,
            |_| Some(0),
            |_| Some(extracted_path.clone()),
            |_, _, output| fs::write(output, INPUT_SRT).map_err(|error| error.to_string()),
            |_, _| Err("network unavailable".to_string()),
        )
        .unwrap();

        assert_eq!(
            outcome,
            CombinedOutcome::TranslationFailed {
                extracted: extracted_path.clone(),
                error: "network unavailable".to_string(),
            }
        );
        assert_eq!(fs::read_to_string(extracted_path).unwrap(), INPUT_SRT);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn extraction_error_prevents_translation() {
        let probe = fake_probe(subtitle_streams(), Rc::new(RefCell::new(Vec::new())));

        let result = extract_and_translate(
            "episode.mkv",
            &probe,
            |_| Some(0),
            |_| Some("episode.srt".to_string()),
            |_, _, _| Err("ffmpeg failed".to_string()),
            |_, _| panic!("must not translate after extraction error"),
        );

        assert_eq!(result.unwrap_err(), "ffmpeg failed");
    }
}
