rust_i18n::i18n!("locales", fallback = "en");

use std::env;

use interfaces::{ffmpeg::SubtitleExtractor, probe::SubtitleProbe};
use rust_i18n::t;
use tui::{MenuChoice, MessageKind};

mod ffmpeg;
mod ffprobe;
mod interfaces;
mod localization;
mod settings;
mod srt;
mod translate;
mod tui;
mod workflow;

use workflow::CombinedOutcome;

enum Action {
    Extract(String),
    Translate,
    ExtractAndTranslate(String),
    Cancelled,
}

fn resolve_action(
    argument: Option<String>,
    show_menu: impl FnOnce() -> MenuChoice,
    prompt_path: impl FnOnce() -> Option<String>,
) -> Option<Action> {
    match argument {
        Some(path) => Some(Action::Extract(path)),
        None => match show_menu() {
            MenuChoice::Extract => Some(
                prompt_path()
                    .map(Action::Extract)
                    .unwrap_or(Action::Cancelled),
            ),
            MenuChoice::Translate => Some(Action::Translate),
            MenuChoice::ExtractAndTranslate => Some(
                prompt_path()
                    .map(Action::ExtractAndTranslate)
                    .unwrap_or(Action::Cancelled),
            ),
            MenuChoice::Exit => None,
        },
    }
}

fn run_action_loop(
    argument: Option<String>,
    mut show_menu: impl FnMut() -> MenuChoice,
    mut prompt_path: impl FnMut() -> Option<String>,
    mut execute_action: impl FnMut(Action),
) {
    if let Some(input) = argument {
        execute_action(Action::Extract(input));
        return;
    }

    loop {
        match resolve_action(None, &mut show_menu, &mut prompt_path) {
            Some(Action::Cancelled) => continue,
            Some(action) => execute_action(action),
            None => break,
        }
    }
}

fn run_translation() {
    let api_key = translate::api_key();

    let Some(input) = tui::prompt_srt_path() else {
        return;
    };

    match translate_srt_interactive(&input, api_key.as_deref(), None) {
        Ok(Some(output)) => {
            tui::show_message(
                MessageKind::Success,
                &translation_success_message(&output, api_key.is_none()),
            );
        }
        Ok(None) => {}
        Err(error) => tui::show_message(MessageKind::Error, &error),
    }
}

fn resolve_translation_source(
    selected_source_language: Option<&str>,
    prompt_source: impl FnOnce() -> Option<String>,
) -> Option<String> {
    match selected_source_language {
        Some(language) => Some(translate::source_language_code(language)),
        None => prompt_source(),
    }
}

fn translate_srt_interactive(
    input: &str,
    api_key: Option<&str>,
    selected_source_language: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(source) =
        resolve_translation_source(selected_source_language, tui::prompt_source_language)
    else {
        return Ok(None);
    };
    let Some(target) = tui::prompt_target_language() else {
        return Ok(None);
    };
    let Some(output) = tui::prompt_translation_output_name(input, &target) else {
        return Ok(None);
    };

    let status = t!(
        "messages.translating",
        input = input,
        source = &source,
        target = &target
    );
    tui::with_status(&status, || {
        translate::translate_file(input, &output, &source, &target, api_key)
    })?;

    Ok(Some(output))
}

fn translation_success_message(output: &str, keyless: bool) -> String {
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

fn combined_success_message(extracted: &str, translated: &str, keyless: bool) -> String {
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

fn show_extraction_only(extracted_srt: Option<&str>) {
    if let Some(output) = extracted_srt {
        tui::show_message(
            MessageKind::Info,
            &t!("messages.extracted_translation_cancelled", output = output),
        );
    }
}

fn run_extraction(input: String, translate_after: bool) {
    if translate_after {
        run_combined_extraction(input);
        return;
    }

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

    let Some(selected_index) = tui::select_subtitle(&subtitle_streams) else {
        return;
    };
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
        Ok(()) => tui::show_message(
            MessageKind::Success,
            &t!("messages.success", output = &output_srt_string),
        ),
        Err(error) => tui::show_message(MessageKind::Error, &error),
    }
}

fn run_combined_extraction(input: String) {
    let probe = ffprobe::Ffprobe;
    let extractor = ffmpeg::Ffmpeg;
    let api_key = translate::api_key();
    let outcome = workflow::extract_and_translate(
        &input,
        &probe,
        tui::select_subtitle,
        |video| Some(tui::prompt_output_name(video)),
        |video, index, output| {
            let status = t!(
                "messages.command",
                input = video,
                index = index,
                output = output
            );
            tui::with_status(&status, || extractor.extract_subtitle(video, index, output))
        },
        |extracted, language| {
            translate_srt_interactive(extracted, api_key.as_deref(), Some(language))
        },
    );

    match outcome {
        Ok(CombinedOutcome::NoSubtitles) => {
            tui::show_message(MessageKind::Info, &t!("messages.no_subtitles"));
        }
        Ok(CombinedOutcome::CancelledBeforeExtraction) => {}
        Ok(CombinedOutcome::ExtractedAndTranslated {
            extracted,
            translated,
        }) => tui::show_message(
            MessageKind::Success,
            &combined_success_message(&extracted, &translated, api_key.is_none()),
        ),
        Ok(CombinedOutcome::TranslationCancelled { extracted }) => {
            show_extraction_only(Some(&extracted));
        }
        Ok(CombinedOutcome::TranslationFailed { extracted, error }) => {
            tui::show_message(
                MessageKind::Error,
                &t!(
                    "messages.extracted_translation_failed",
                    output = extracted,
                    error = error
                ),
            );
        }
        Err(error) => tui::show_message(MessageKind::Error, &error),
    }
}

fn main() {
    let _ = dotenvy::dotenv();
    localization::initialize();

    run_action_loop(
        env::args().nth(1),
        tui::main_menu,
        tui::prompt_input_path,
        |action| match action {
            Action::Extract(input) => run_extraction(input, false),
            Action::Translate => run_translation(),
            Action::ExtractAndTranslate(input) => run_extraction(input, true),
            Action::Cancelled => {}
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{Action, MenuChoice, resolve_action, resolve_translation_source, run_action_loop};
    use std::{cell::Cell, cell::RefCell};

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
    fn combined_flow_uses_selected_track_language_without_prompting() {
        let prompt_called = Cell::new(false);
        let source = resolve_translation_source(Some("eng"), || {
            prompt_called.set(true);
            Some("fr".to_string())
        });

        assert_eq!(source.as_deref(), Some("en"));
        assert!(!prompt_called.get());
    }

    #[test]
    fn standalone_translation_still_prompts_for_source_language() {
        let prompt_called = Cell::new(false);
        let source = resolve_translation_source(None, || {
            prompt_called.set(true);
            Some("fr".to_string())
        });

        assert_eq!(source.as_deref(), Some("fr"));
        assert!(prompt_called.get());
    }

    #[test]
    fn cancelled_video_prompt_is_an_action_that_returns_to_menu() {
        assert!(matches!(
            resolve_action(None, || MenuChoice::Extract, || None),
            Some(Action::Cancelled)
        ));
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

    #[test]
    fn completed_actions_return_to_menu_until_user_selects_exit() {
        let menu_calls = Cell::new(0);
        let mut choices =
            [MenuChoice::Extract, MenuChoice::Translate, MenuChoice::Exit].into_iter();
        let actions = RefCell::new(Vec::new());

        run_action_loop(
            None,
            || {
                menu_calls.set(menu_calls.get() + 1);
                choices.next().unwrap()
            },
            || Some("episode.srt".to_string()),
            |action| match action {
                Action::Extract(path) => actions.borrow_mut().push(format!("extract:{path}")),
                Action::Translate => actions.borrow_mut().push("translate".to_string()),
                Action::ExtractAndTranslate(path) => {
                    actions.borrow_mut().push(format!("combined:{path}"));
                }
                Action::Cancelled => actions.borrow_mut().push("cancelled".to_string()),
            },
        );

        assert_eq!(menu_calls.get(), 3);
        assert_eq!(*actions.borrow(), ["extract:episode.srt", "translate"]);
    }

    #[test]
    fn argument_runs_once_without_opening_the_menu() {
        let menu_calls = Cell::new(0);
        let actions = RefCell::new(Vec::new());

        run_action_loop(
            Some("video.mkv".to_string()),
            || {
                menu_calls.set(menu_calls.get() + 1);
                MenuChoice::Exit
            },
            || panic!("CLI argument must skip the path prompt"),
            |action| match action {
                Action::Extract(path) => actions.borrow_mut().push(path),
                _ => panic!("expected CLI extraction action"),
            },
        );

        assert_eq!(menu_calls.get(), 0);
        assert_eq!(*actions.borrow(), ["video.mkv"]);
    }
}
