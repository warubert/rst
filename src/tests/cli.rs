use super::{CliCommand, CliParse, find_subtitle_by_language, parse_args};
use crate::interfaces::probe::Subtitle;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn parses_each_action_with_its_required_arguments() {
    let cases = [
        (
            args(&[
                "--action",
                "extract",
                "--input",
                "video.mkv",
                "--subtitle-language",
                "pt",
                "--output",
                "captions.srt",
            ]),
            CliCommand::Extract {
                input: "video.mkv".into(),
                subtitle_language: "pt".into(),
                output: "captions.srt".into(),
            },
        ),
        (
            args(&[
                "--action",
                "translate",
                "--input",
                "captions.srt",
                "--source-language",
                "en",
                "--target-language",
                "pt",
                "--output",
                "captions-pt.srt",
            ]),
            CliCommand::Translate {
                input: "captions.srt".into(),
                source_language: "en".into(),
                target_language: "pt".into(),
                output: "captions-pt.srt".into(),
            },
        ),
        (
            args(&[
                "--action",
                "extract-and-translate",
                "--input",
                "video.mkv",
                "--subtitle-language",
                "pt",
                "--target-language",
                "pt",
                "--output",
                "captions.srt",
                "--translated-output",
                "captions-pt.srt",
            ]),
            CliCommand::ExtractAndTranslate {
                input: "video.mkv".into(),
                subtitle_language: "pt".into(),
                target_language: "pt".into(),
                output: "captions.srt".into(),
                translated_output: "captions-pt.srt".into(),
            },
        ),
        (
            args(&[
                "--action",
                "transcribe",
                "--input",
                "video.mkv",
                "--language",
                "en",
                "--output",
                "captions.srt",
            ]),
            CliCommand::Transcribe {
                input: "video.mkv".into(),
                language: "en".into(),
                output: "captions.srt".into(),
            },
        ),
        (
            args(&[
                "--action",
                "transcribe-and-translate",
                "--input",
                "video.mkv",
                "--language",
                "en",
                "--target-language",
                "pt",
                "--output",
                "captions.srt",
                "--translated-output",
                "captions-pt.srt",
            ]),
            CliCommand::TranscribeAndTranslate {
                input: "video.mkv".into(),
                language: "en".into(),
                target_language: "pt".into(),
                output: "captions.srt".into(),
                translated_output: "captions-pt.srt".into(),
            },
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(parse_args(&input), Ok(CliParse::Command(expected)));
    }
}

#[test]
fn rejects_missing_action_specific_arguments() {
    let error = parse_args(&args(&[
        "--action",
        "extract",
        "--input",
        "video.mkv",
        "--output",
        "captions.srt",
    ]))
    .unwrap_err();

    assert_eq!(error, "Missing required option: --subtitle-language");
}

#[test]
fn rejects_options_not_used_by_the_selected_action() {
    let error = parse_args(&args(&[
        "--action",
        "translate",
        "--input",
        "captions.srt",
        "--source-language",
        "en",
        "--target-language",
        "pt",
        "--output",
        "translated.srt",
        "--subtitle-language",
        "pt",
    ]))
    .unwrap_err();

    assert!(error.starts_with("Option --subtitle-language is not used"));
}

#[test]
fn supports_help_and_rejects_legacy_positional_arguments() {
    assert_eq!(parse_args(&args(&["--help"])), Ok(CliParse::Help));
    assert!(parse_args(&args(&["video.mkv"])).is_err());
}

#[test]
fn rejects_combined_actions_that_would_overwrite_the_original_srt() {
    let error = parse_args(&args(&[
        "--action",
        "transcribe-and-translate",
        "--input",
        "video.mkv",
        "--language",
        "en",
        "--target-language",
        "pt",
        "--output",
        "captions.srt",
        "--translated-output",
        "captions.srt",
    ]))
    .unwrap_err();

    assert_eq!(
        error,
        "--output and --translated-output must be different paths"
    );
}

#[test]
fn subtitle_language_lookup_matches_language_and_region() {
    let subtitles = [
        Subtitle {
            index: 1,
            language: "en".to_string(),
            is_sdh: false,
        },
        Subtitle {
            index: 4,
            language: "pt-BR".to_string(),
            is_sdh: false,
        },
    ];

    assert_eq!(
        find_subtitle_by_language(&subtitles, "pt").unwrap().index,
        4
    );
    assert_eq!(
        find_subtitle_by_language(&subtitles, "POR").unwrap().index,
        4
    );
    assert!(find_subtitle_by_language(&subtitles, "es").is_none());
}

#[test]
fn subtitle_language_lookup_prefers_an_exact_match() {
    let subtitles = [
        Subtitle {
            index: 2,
            language: "pt-BR".to_string(),
            is_sdh: false,
        },
        Subtitle {
            index: 5,
            language: "pt".to_string(),
            is_sdh: false,
        },
    ];

    assert_eq!(
        find_subtitle_by_language(&subtitles, "pt").unwrap().index,
        5
    );
}
