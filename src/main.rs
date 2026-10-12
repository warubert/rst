rust_i18n::i18n!("locales", fallback = "en");

use std::env;

use interfaces::{ffmpeg::SubtitleExtractor, probe::SubtitleProbe};
use tui::MenuChoice;

mod ffmpeg;
mod ffprobe;
mod interfaces;
mod localization;
mod srt;
mod terminal;
mod translate;
mod tui;

enum Action {
    Extract(String),
    Translate,
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
            MenuChoice::Exit => None,
        },
    }
}

fn run_translation() {
    let api_key = match translate::api_key() {
        Ok(key) => key,
        Err(error) => {
            terminal::print_error(&error);
            return;
        }
    };

    let Some(input) = tui::prompt_srt_path() else {
        return;
    };
    let Some(source) = tui::prompt_source_language() else {
        return;
    };
    let Some(target) = tui::prompt_target_language() else {
        return;
    };
    let Some(output) = tui::prompt_translation_output_name(&input, &target) else {
        return;
    };

    terminal::print_translating(&input, &source, &target);

    match translate::translate_file(&input, &output, &source, &target, &api_key) {
        Ok(()) => terminal::print_translation_success(&output),
        Err(error) => terminal::print_error(&error),
    }
}

fn main() {
    let _ = dotenvy::dotenv();
    localization::initialize();

    let action = resolve_action(env::args().nth(1), tui::main_menu, tui::prompt_input_path);
    let input = match action {
        Some(Action::Extract(input)) => input,
        Some(Action::Translate) => return run_translation(),
        None => return,
    };

    let probe = ffprobe::Ffprobe;
    let subtitle_streams = match probe.get_subtitle_streams(&input) {
        Ok(streams) => streams,
        Err(error) => {
            terminal::print_error(&error);
            return;
        }
    };

    if subtitle_streams.is_empty() {
        terminal::print_no_subtitles_found();
        return;
    }

    let selected_index = tui::select_subtitle(&subtitle_streams);
    let selected_subtitle = &subtitle_streams[selected_index];
    let output_srt_string = tui::prompt_output_name(&input);

    terminal::print_ffmpeg_command(&input, selected_subtitle.index, &output_srt_string);

    let extractor = ffmpeg::Ffmpeg;
    if let Err(error) =
        extractor.extract_subtitle(&input, selected_subtitle.index, &output_srt_string)
    {
        terminal::print_error(&error);
        return;
    }

    terminal::print_success(&output_srt_string);
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
