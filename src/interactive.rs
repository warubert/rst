use std::{cell::RefCell, path::PathBuf};

use crate::{
    ffmpeg, ffprobe,
    interfaces::{
        ffmpeg::SubtitleExtractor,
        probe::SubtitleProbe,
        whisper::{SubtitleTranscriber, TranscriptionEvent},
    },
    messages, translate, tui,
    tui::{MenuChoice, MessageKind},
    whisper, workflow,
    workflow::CombinedOutcome,
};
use rust_i18n::t;

enum Action {
    Extract(String),
    Translate,
    ExtractAndTranslate(String),
    Transcribe,
    TranscribeAndTranslate,
    Cancelled,
}

fn resolve_action(
    show_menu: impl FnOnce() -> MenuChoice,
    prompt_path: impl FnOnce() -> Option<String>,
) -> Option<Action> {
    match show_menu() {
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
        MenuChoice::Transcribe => Some(Action::Transcribe),
        MenuChoice::TranscribeAndTranslate => Some(Action::TranscribeAndTranslate),
        MenuChoice::Exit => None,
    }
}

fn run_action_loop(
    mut show_menu: impl FnMut() -> MenuChoice,
    mut prompt_path: impl FnMut() -> Option<String>,
    mut execute_action: impl FnMut(Action),
) {
    loop {
        match resolve_action(&mut show_menu, &mut prompt_path) {
            Some(Action::Cancelled) => continue,
            Some(action) => execute_action(action),
            None => break,
        }
    }
}

fn run_translation() {
    let api_key = translate::api_key();
    let mut session = tui::TuiSession::new();

    let Some(input) = session.prompt_srt_path() else {
        return;
    };

    match translate_srt_interactive(&mut session, &input, api_key.as_deref(), None) {
        Ok(Some(output)) => {
            session.show_message(
                MessageKind::Success,
                &messages::translation_success_message(&output, api_key.is_none()),
            );
        }
        Ok(None) => {}
        Err(error) => session.show_message(MessageKind::Error, &error),
    }
}

fn prompt_transcription_inputs(
    session: &mut tui::TuiSession,
    transcriber: &impl SubtitleTranscriber,
) -> Option<(String, String, String)> {
    let languages = match session.with_status(&t!("whisper.checking"), || {
        transcriber.supported_languages()
    }) {
        Ok(languages) => languages,
        Err(error) => {
            session.show_message(MessageKind::Info, &t!("whisper.unavailable", error = error));
            return None;
        }
    };

    let input = session.prompt_input_path()?;
    let language = session.prompt_whisper_language(&languages)?;
    let output = session.prompt_output_name(&input);

    Some((input, language, output))
}

fn transcribe_with_progress(
    session: &mut tui::TuiSession,
    transcriber: &impl SubtitleTranscriber,
    input: &str,
    language: &str,
    output: &str,
) -> Result<PathBuf, String> {
    session.show_progress(
        &t!("whisper.progress", input = input, language = language),
        0,
    );
    transcriber.transcribe(input, language, output, &mut |event| match event {
        TranscriptionEvent::Progress(percent) => session.show_progress(
            &t!("whisper.progress", input = input, language = language),
            percent,
        ),
        TranscriptionEvent::CpuFallback => session.show_progress(
            &t!("whisper.cpu_fallback", input = input, language = language),
            0,
        ),
    })
}

fn run_transcription() {
    let mut session = tui::TuiSession::new();
    let transcriber = whisper::Whisper;
    let Some((input, language, output)) = prompt_transcription_inputs(&mut session, &transcriber)
    else {
        return;
    };

    let result = transcribe_with_progress(&mut session, &transcriber, &input, &language, &output);

    match result {
        Ok(output) => {
            session.show_progress(
                &t!("whisper.progress", input = &input, language = &language),
                100,
            );
            session.show_message(
                MessageKind::Success,
                &t!("whisper.success", output = output.display()),
            );
        }
        Err(error) => session.show_message(MessageKind::Error, &error),
    }
}

fn run_whisper_and_translate() {
    let mut session = tui::TuiSession::new();
    let transcriber = whisper::Whisper;
    let api_key = translate::api_key();
    let Some((input, language, output)) = prompt_transcription_inputs(&mut session, &transcriber)
    else {
        return;
    };

    let transcribed =
        match transcribe_with_progress(&mut session, &transcriber, &input, &language, &output) {
            Ok(output) => output,
            Err(error) => {
                session.show_message(MessageKind::Error, &error);
                return;
            }
        };
    let transcribed = transcribed.to_string_lossy().into_owned();

    match translate_srt_interactive(
        &mut session,
        &transcribed,
        api_key.as_deref(),
        Some(&language),
    ) {
        Ok(Some(translated)) => session.show_message(
            MessageKind::Success,
            &messages::whisper_translation_success_message(
                &transcribed,
                &translated,
                api_key.is_none(),
            ),
        ),
        Ok(None) => session.show_message(
            MessageKind::Info,
            &t!("whisper.translation_cancelled", output = transcribed),
        ),
        Err(error) => session.show_message(
            MessageKind::Error,
            &t!(
                "whisper.translation_failed",
                output = transcribed,
                error = error
            ),
        ),
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
    session: &mut tui::TuiSession,
    input: &str,
    api_key: Option<&str>,
    selected_source_language: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(source) = resolve_translation_source(selected_source_language, || {
        session.prompt_source_language()
    }) else {
        return Ok(None);
    };
    let Some(target) = session.prompt_target_language() else {
        return Ok(None);
    };
    let Some(output) = session.prompt_translation_output_name(input, &target) else {
        return Ok(None);
    };

    let status = t!(
        "messages.translating",
        input = input,
        source = &source,
        target = &target
    );
    session.with_status(&status, || {
        translate::translate_file(input, &output, &source, &target, api_key)
    })?;

    Ok(Some(output))
}

fn run_extraction(input: String) {
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

    let mut session = tui::TuiSession::new();
    let Some(selected_index) = session.select_subtitle(&subtitle_streams) else {
        return;
    };
    let selected_subtitle = &subtitle_streams[selected_index];
    let output_srt_string = session.prompt_output_name(&input);

    let status = t!(
        "messages.extracting",
        input = &input,
        index = selected_subtitle.index,
        output = &output_srt_string
    );
    let extractor = ffmpeg::Ffmpeg;
    let result = session.with_status(&status, || {
        extractor.extract_subtitle(&input, selected_subtitle.index, &output_srt_string)
    });

    match result {
        Ok(()) => session.show_message(
            MessageKind::Success,
            &t!("messages.success", output = &output_srt_string),
        ),
        Err(error) => session.show_message(MessageKind::Error, &error),
    }
}

fn run_combined_extraction(input: String) {
    let probe = ffprobe::Ffprobe;
    let extractor = ffmpeg::Ffmpeg;
    let api_key = translate::api_key();
    let session = RefCell::new(tui::TuiSession::new());
    let outcome = workflow::extract_and_translate(
        &input,
        &probe,
        |streams| session.borrow_mut().select_subtitle(streams),
        |video| Some(session.borrow_mut().prompt_output_name(video)),
        |video, index, output| {
            let status = t!(
                "messages.extracting",
                input = video,
                index = index,
                output = output
            );
            session
                .borrow_mut()
                .with_status(&status, || extractor.extract_subtitle(video, index, output))
        },
        |extracted, language| {
            translate_srt_interactive(
                &mut session.borrow_mut(),
                extracted,
                api_key.as_deref(),
                Some(language),
            )
        },
    );

    match outcome {
        Ok(CombinedOutcome::NoSubtitles) => {
            session
                .borrow_mut()
                .show_message(MessageKind::Info, &t!("messages.no_subtitles"));
        }
        Ok(CombinedOutcome::CancelledBeforeExtraction) => {}
        Ok(CombinedOutcome::ExtractedAndTranslated {
            extracted,
            translated,
        }) => session.borrow_mut().show_message(
            MessageKind::Success,
            &messages::combined_success_message(&extracted, &translated, api_key.is_none()),
        ),
        Ok(CombinedOutcome::TranslationCancelled { extracted }) => {
            session.borrow_mut().show_message(
                MessageKind::Info,
                &t!(
                    "messages.extracted_translation_cancelled",
                    output = extracted
                ),
            );
        }
        Ok(CombinedOutcome::TranslationFailed { extracted, error }) => {
            session.borrow_mut().show_message(
                MessageKind::Error,
                &t!(
                    "messages.extracted_translation_failed",
                    output = extracted,
                    error = error
                ),
            );
        }
        Err(error) => session
            .borrow_mut()
            .show_message(MessageKind::Error, &error),
    }
}

pub fn run() {
    run_action_loop(
        tui::main_menu,
        tui::prompt_input_path,
        |action| match action {
            Action::Extract(input) => run_extraction(input),
            Action::Translate => run_translation(),
            Action::ExtractAndTranslate(input) => run_combined_extraction(input),
            Action::Transcribe => run_transcription(),
            Action::TranscribeAndTranslate => run_whisper_and_translate(),
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
    fn extract_choice_prompts_for_a_video_path() {
        let action = resolve_action(|| MenuChoice::Extract, || Some("video.mkv".to_string()));
        assert_eq!(extract_path(action).as_deref(), Some("video.mkv"));
    }

    #[test]
    fn combined_choice_prompts_for_video_and_selects_both_operations() {
        let action = resolve_action(
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
            resolve_action(|| MenuChoice::Extract, || None),
            Some(Action::Cancelled)
        ));
    }

    #[test]
    fn translate_choice_starts_translation_without_video_prompt() {
        let prompt_called = Cell::new(false);
        let action = resolve_action(
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
    fn transcribe_choice_starts_the_whisper_flow_without_shared_path_prompt() {
        let prompt_called = Cell::new(false);
        let action = resolve_action(
            || MenuChoice::Transcribe,
            || {
                prompt_called.set(true);
                Some("video.mkv".to_string())
            },
        );

        assert!(matches!(action, Some(Action::Transcribe)));
        assert!(!prompt_called.get());
    }

    #[test]
    fn whisper_and_translate_choice_starts_combined_flow_without_shared_path_prompt() {
        let prompt_called = Cell::new(false);
        let action = resolve_action(
            || MenuChoice::TranscribeAndTranslate,
            || {
                prompt_called.set(true);
                Some("video.mkv".to_string())
            },
        );

        assert!(matches!(action, Some(Action::TranscribeAndTranslate)));
        assert!(!prompt_called.get());
    }

    #[test]
    fn leaving_menu_does_not_prompt_for_a_path() {
        let prompt_called = Cell::new(false);
        let action = resolve_action(
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
                Action::Transcribe => actions.borrow_mut().push("transcribe".to_string()),
                Action::TranscribeAndTranslate => actions
                    .borrow_mut()
                    .push("transcribe-and-translate".to_string()),
                Action::Cancelled => actions.borrow_mut().push("cancelled".to_string()),
            },
        );

        assert_eq!(menu_calls.get(), 3);
        assert_eq!(*actions.borrow(), ["extract:episode.srt", "translate"]);
    }
}
