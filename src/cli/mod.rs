use std::path::PathBuf;

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

mod args;

pub use args::{CliCommand, CliParse, parse_args};

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
#[path = "../tests/cli.rs"]
mod tests;
