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
    let mut choices = [MenuChoice::Extract, MenuChoice::Translate, MenuChoice::Exit].into_iter();
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
