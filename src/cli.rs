use std::{collections::HashMap, path::PathBuf};

use crate::{
    ffmpeg::{self},
    ffprobe,
    interfaces::{
        ffmpeg::SubtitleExtractor,
        probe::{Subtitle, SubtitleProbe},
        whisper::{SubtitleTranscriber, TranscriptionEvent},
    },
    messages, translate, whisper,
};
use rust_i18n::t;

pub const USAGE: &str = concat!(
    "Usage:\n",
    "  rst --action <ACTION> [OPTIONS]\n\n",
    "Actions and required options:\n",
    "  extract                 --input VIDEO --subtitle-language CODE --output SRT\n",
    "  translate               --input SRT --source-language CODE --target-language CODE --output SRT\n",
    "  extract-and-translate   --input VIDEO --subtitle-language CODE --target-language CODE --output SRT --translated-output SRT\n",
    "  transcribe              --input VIDEO --language CODE --output SRT\n",
    "  transcribe-and-translate --input VIDEO --language CODE --target-language CODE --output SRT --translated-output SRT\n\n",
    "Run rst without arguments to open the interactive menu. Use --help to show this help.\n",
);

#[derive(Debug, PartialEq, Eq)]
pub enum CliParse {
    Help,
    Command(CliCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Extract {
        input: String,
        subtitle_language: String,
        output: String,
    },
    Translate {
        input: String,
        source_language: String,
        target_language: String,
        output: String,
    },
    ExtractAndTranslate {
        input: String,
        subtitle_language: String,
        target_language: String,
        output: String,
        translated_output: String,
    },
    Transcribe {
        input: String,
        language: String,
        output: String,
    },
    TranscribeAndTranslate {
        input: String,
        language: String,
        target_language: String,
        output: String,
        translated_output: String,
    },
}

pub fn parse_args(args: &[String]) -> Result<CliParse, String> {
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        return Ok(CliParse::Help);
    }

    let mut options = HashMap::new();
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        let Some(name) = argument.strip_prefix("--") else {
            return Err(format!("Unexpected positional argument: {argument}"));
        };
        if !matches!(
            name,
            "action"
                | "input"
                | "subtitle-language"
                | "source-language"
                | "target-language"
                | "language"
                | "output"
                | "translated-output"
        ) {
            return Err(format!("Unknown option: {argument}"));
        }
        let value = arguments
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("Missing value for {argument}"))?;
        if options.insert(name, value.clone()).is_some() {
            return Err(format!("Option {argument} was provided more than once"));
        }
    }

    let action = required(&mut options, "action")?;
    let command = match action.as_str() {
        "extract" => CliCommand::Extract {
            input: required(&mut options, "input")?,
            subtitle_language: required(&mut options, "subtitle-language")?,
            output: required(&mut options, "output")?,
        },
        "translate" => CliCommand::Translate {
            input: required(&mut options, "input")?,
            source_language: required(&mut options, "source-language")?,
            target_language: required(&mut options, "target-language")?,
            output: required(&mut options, "output")?,
        },
        "extract-and-translate" => {
            let output = required(&mut options, "output")?;
            let translated_output = required(&mut options, "translated-output")?;
            let (output, translated_output) = distinct_output(output, translated_output)?;
            CliCommand::ExtractAndTranslate {
                input: required(&mut options, "input")?,
                subtitle_language: required(&mut options, "subtitle-language")?,
                target_language: required(&mut options, "target-language")?,
                output,
                translated_output,
            }
        }
        "transcribe" => CliCommand::Transcribe {
            input: required(&mut options, "input")?,
            language: required(&mut options, "language")?,
            output: required(&mut options, "output")?,
        },
        "transcribe-and-translate" => {
            let output = required(&mut options, "output")?;
            let translated_output = required(&mut options, "translated-output")?;
            let (output, translated_output) = distinct_output(output, translated_output)?;
            CliCommand::TranscribeAndTranslate {
                input: required(&mut options, "input")?,
                language: required(&mut options, "language")?,
                target_language: required(&mut options, "target-language")?,
                output,
                translated_output,
            }
        }
        _ => return Err(format!("Unknown action: {action}")),
    };

    if let Some((name, _)) = options.into_iter().next() {
        return Err(format!("Option --{name} is not used by action {action}"));
    }

    Ok(CliParse::Command(command))
}

fn required(options: &mut HashMap<&str, String>, name: &'static str) -> Result<String, String> {
    options
        .remove(name)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing required option: --{name}"))
}

fn distinct_output(output: String, translated_output: String) -> Result<(String, String), String> {
    if output == translated_output {
        return Err("--output and --translated-output must be different paths".to_string());
    }
    Ok((output, translated_output))
}

fn normalized_subtitle_language(language: &str) -> String {
    translate::source_language_code(language).to_ascii_lowercase()
}

fn subtitle_language_matches(available: &str, requested: &str) -> bool {
    let available = normalized_subtitle_language(available);
    let requested = normalized_subtitle_language(requested);
    available == requested
        || (available != "auto" && available.starts_with(&format!("{requested}-")))
}

fn find_subtitle_by_language<'a>(
    subtitles: &'a [Subtitle],
    requested: &str,
) -> Option<&'a Subtitle> {
    subtitles
        .iter()
        .find(|subtitle| {
            normalized_subtitle_language(&subtitle.language)
                == normalized_subtitle_language(requested)
        })
        .or_else(|| {
            subtitles
                .iter()
                .find(|subtitle| subtitle_language_matches(&subtitle.language, requested))
        })
}

fn subtitle_for_language(input: &str, language: &str) -> Result<(i64, String), String> {
    let subtitles = ffprobe::Ffprobe.get_subtitle_streams(input)?;
    if let Some(subtitle) = find_subtitle_by_language(&subtitles, language) {
        return Ok((subtitle.index, subtitle.language.clone()));
    }

    let available = subtitles
        .iter()
        .map(|subtitle| subtitle.language.as_str())
        .collect::<Vec<_>>();
    let available = if available.is_empty() {
        "none".to_string()
    } else {
        available.join(", ")
    };
    Err(format!(
        "No subtitle track with language '{language}' was found in {input}. Available languages: {available}"
    ))
}

fn transcribe(input: &str, language: &str, output: &str) -> Result<PathBuf, String> {
    let transcriber = whisper::Whisper;
    let languages = transcriber.supported_languages()?;
    if !languages
        .iter()
        .any(|supported| supported.code() == language)
    {
        return Err(format!("Whisper does not support language code {language}"));
    }

    let result = transcriber.transcribe(input, language, output, &mut |event| match event {
        TranscriptionEvent::Progress(percent) => {
            eprint!("\rTranscribing {input} ({language}): {percent}%")
        }
        TranscriptionEvent::CpuFallback => {
            eprintln!("\nCUDA is unavailable for this GPU; retrying on CPU.")
        }
    });
    eprintln!();
    result
}

pub fn run(command: CliCommand) -> Result<(), String> {
    match command {
        CliCommand::Extract {
            input,
            subtitle_language,
            output,
        } => {
            let (subtitle_index, _) = subtitle_for_language(&input, &subtitle_language)?;
            ffmpeg::Ffmpeg.extract_subtitle(&input, subtitle_index, &output)?;
            println!("{}", t!("messages.success", output = output));
        }
        CliCommand::Translate {
            input,
            source_language,
            target_language,
            output,
        } => {
            let api_key = translate::api_key();
            translate::translate_file(
                &input,
                &output,
                &source_language,
                &target_language,
                api_key.as_deref(),
            )?;
            println!(
                "{}",
                messages::translation_success_message(&output, api_key.is_none())
            );
        }
        CliCommand::ExtractAndTranslate {
            input,
            subtitle_language,
            target_language,
            output,
            translated_output,
        } => {
            let (subtitle_index, subtitle_language) =
                subtitle_for_language(&input, &subtitle_language)?;
            ffmpeg::Ffmpeg.extract_subtitle(&input, subtitle_index, &output)?;
            let source_language = translate::source_language_code(&subtitle_language);
            let api_key = translate::api_key();
            if let Err(error) = translate::translate_file(
                &output,
                &translated_output,
                &source_language,
                &target_language,
                api_key.as_deref(),
            ) {
                return Err(format!("{error}\nExtracted subtitle was saved to {output}"));
            }
            println!(
                "{}",
                messages::combined_success_message(&output, &translated_output, api_key.is_none())
            );
        }
        CliCommand::Transcribe {
            input,
            language,
            output,
        } => {
            let output = transcribe(&input, &language, &output)?;
            println!("{}", t!("whisper.success", output = output.display()));
        }
        CliCommand::TranscribeAndTranslate {
            input,
            language,
            target_language,
            output,
            translated_output,
        } => {
            let transcribed = transcribe(&input, &language, &output)?;
            let transcribed = transcribed.to_string_lossy().into_owned();
            let api_key = translate::api_key();
            if let Err(error) = translate::translate_file(
                &transcribed,
                &translated_output,
                &language,
                &target_language,
                api_key.as_deref(),
            ) {
                return Err(format!("{error}\nTranscription was saved to {transcribed}"));
            }
            println!(
                "{}",
                messages::whisper_translation_success_message(
                    &transcribed,
                    &translated_output,
                    api_key.is_none()
                )
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
