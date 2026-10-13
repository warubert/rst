rust_i18n::i18n!("locales", fallback = "en");

use std::env;

use interfaces::{ffmpeg::SubtitleExtractor, probe::SubtitleProbe};
use rust_i18n::t;
use tui::{MenuChoice, MessageKind};

mod ffmpeg;
mod ffprobe;
mod interfaces;
mod localization;
mod srt;
mod translate;
mod tui;

enum Action {
    Extract(String),
    Translate,
    ExtractAndTranslate(String),
}

fn resolve_action(
    argument: Option<String>,
    show_menu: impl FnOnce() -> MenuChoice,
    prompt_path: impl FnOnce() -> Option<String>,
) -> Option<Action> {
    match argument {
        Some(path) => Some(Action::Extract(path)),
        None => match show_menu() {
            MenuChoice::Extract => prompt_path().map(Action::Extract),
            MenuChoice::Translate => Some(Action::Translate),
            MenuChoice::ExtractAndTranslate => prompt_path().map(Action::ExtractAndTranslate),
            MenuChoice::Exit => None,
        },
    }
}

fn run_translation() {
    let api_key = translate::api_key();

    let Some(input) = tui::prompt_srt_path() else {
        return;
    };
    run_translation_for_srt(&input, api_key, None);
}

fn run_translation_for_srt(input: &str, api_key: Option<String>, extracted_srt: Option<&str>) {
    let Some(source) = tui::prompt_source_language() else {
        show_extraction_only(extracted_srt);
        return;
    };
    let Some(target) = tui::prompt_target_language() else {
        show_extraction_only(extracted_srt);
        return;
    };
    let Some(output) = tui::prompt_translation_output_name(input, &target) else {
        show_extraction_only(extracted_srt);
        return;
    };

    let status = t!(
        "messages.translating",
        input = input,
        source = &source,
        target = &target
    );
    let result = tui::with_status(&status, || {
        translate::translate_file(input, &output, &source, &target, api_key.as_deref())
    });

    match result {
        Ok(()) => {
            let mut message = match extracted_srt {
                Some(extracted) => t!(
                    "messages.extract_translate_success",
                    extracted = extracted,
                    translated = &output
                )
                .to_string(),
                None => t!("messages.translation_success", output = &output).to_string(),
            };
            if api_key.is_none() {
                message.push_str("\n\n");
                message.push_str(&t!(
                    "messages.translation_without_key",
                    name = translate::API_KEY_ENV
                ));
            }
            tui::show_message(MessageKind::Success, &message);
        }
        Err(error) => {
            let message = match extracted_srt {
                Some(extracted) => t!(
                    "messages.extracted_translation_failed",
                    output = extracted,
                    error = error
                )
                .to_string(),
                None => error,
            };
            tui::show_message(MessageKind::Error, &message);
        }
    }
}

fn show_extraction_only(extracted_srt: Option<&str>) {
    if let Some(output) = extracted_srt {
        tui::show_message(
            MessageKind::Info,
            &t!("messages.extracted_translation_cancelled", output = output),
        );
    }
}

fn run_extraction(input: String, translate_after: bool) {
    let probe = ffprobe::Ffprobe;
    let subtitle_streams = match probe.get_subtitle_streams(&input) {
        Ok(streams) => streams,
        Err(error) => {
            tui::show_message(MessageKind::Error, &error);
            return;
        }
    };

    if subtitle_streams.is_empty() {
        tui::show_message(MessageKind::Info, &t!("messages.no_subtitles"));
        return;
    }

    let selected_index = tui::select_subtitle(&subtitle_streams);
    let selected_subtitle = &subtitle_streams[selected_index];
    let output_srt_string = tui::prompt_output_name(&input);

    let status = t!(
        "messages.command",
        input = &input,
        index = selected_subtitle.index,
        output = &output_srt_string
    );
    let extractor = ffmpeg::Ffmpeg;
    let result = tui::with_status(&status, || {
        extractor.extract_subtitle(&input, selected_subtitle.index, &output_srt_string)
    });

    match result {
        Ok(()) if translate_after => {
            run_translation_for_srt(
                &output_srt_string,
                translate::api_key(),
                Some(&output_srt_string),
            );
        }
        Ok(()) => tui::show_message(
            MessageKind::Success,
            &t!("messages.success", output = &output_srt_string),
        ),
        Err(error) => tui::show_message(MessageKind::Error, &error),
    }
}

fn main() {
    let _ = dotenvy::dotenv();
    localization::initialize();

    let action = resolve_action(env::args().nth(1), tui::main_menu, tui::prompt_input_path);
    match action {
        Some(Action::Extract(input)) => run_extraction(input, false),
        Some(Action::Translate) => run_translation(),
        Some(Action::ExtractAndTranslate(input)) => run_extraction(input, true),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, MenuChoice, resolve_action};
    use std::cell::Cell;

    fn extract_path(action: Option<Action>) -> Option<String> {
        match action {
            Some(Action::Extract(path)) => Some(path),
            _ => None,
        }
    }

    #[test]
    fn argument_skips_menu_and_path_prompt() {
        let menu_called = Cell::new(false);
        let prompt_called = Cell::new(false);

        let action = resolve_action(
            Some("video.mkv".to_string()),
            || {
                menu_called.set(true);
                MenuChoice::Exit
            },
            || {
                prompt_called.set(true);
                None
            },
        );

        assert_eq!(extract_path(action).as_deref(), Some("video.mkv"));
        assert!(!menu_called.get());
        assert!(!prompt_called.get());
    }

    #[test]
    fn extract_choice_prompts_for_a_video_path() {
        let action = resolve_action(
            None,
            || MenuChoice::Extract,
            || Some("video.mkv".to_string()),
        );
        assert_eq!(extract_path(action).as_deref(), Some("video.mkv"));
    }

    #[test]
    fn combined_choice_prompts_for_video_and_selects_both_operations() {
        let action = resolve_action(
            None,
            || MenuChoice::ExtractAndTranslate,
            || Some("video.mkv".to_string()),
        );

        assert!(matches!(action, Some(Action::ExtractAndTranslate(path)) if path == "video.mkv"));
    }

    #[test]
    fn cancelled_video_prompt_does_nothing() {
        assert!(resolve_action(None, || MenuChoice::Extract, || None).is_none());
    }

    #[test]
    fn translate_choice_starts_translation_without_video_prompt() {
        let prompt_called = Cell::new(false);
        let action = resolve_action(
            None,
            || MenuChoice::Translate,
            || {
                prompt_called.set(true);
                Some("video.mkv".to_string())
            },
        );

        assert!(matches!(action, Some(Action::Translate)));
        assert!(!prompt_called.get());
    }

    #[test]
    fn leaving_menu_does_not_prompt_for_a_path() {
        let prompt_called = Cell::new(false);
        let action = resolve_action(
            None,
            || MenuChoice::Exit,
            || {
                prompt_called.set(true);
                Some("video.mkv".to_string())
            },
        );

        assert!(action.is_none());
        assert!(!prompt_called.get());
    }
}
