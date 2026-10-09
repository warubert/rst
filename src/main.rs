rust_i18n::i18n!("locales", fallback = "en");

use std::env;

use interfaces::{ffmpeg::SubtitleExtractor, probe::SubtitleProbe};

mod ffmpeg;
mod ffprobe;
mod interfaces;
mod localization;
mod terminal;
mod tui;

fn resolve_input(
    argument: Option<String>,
    show_menu: impl FnOnce() -> bool,
    prompt_path: impl FnOnce() -> Option<String>,
) -> Option<String> {
    match argument {
        Some(path) => Some(path),
        None if show_menu() => prompt_path(),
        None => None,
    }
}

fn main() {
    localization::initialize();

    let input = resolve_input(env::args().nth(1), tui::main_menu, tui::prompt_input_path);
    let Some(input) = input else {
        return;
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
    use super::resolve_input;
    use std::cell::Cell;

    #[test]
    fn argument_skips_menu_and_path_prompt() {
        let menu_called = Cell::new(false);
        let prompt_called = Cell::new(false);

        let input = resolve_input(
            Some("video.mkv".to_string()),
            || {
                menu_called.set(true);
                false
            },
            || {
                prompt_called.set(true);
                None
            },
        );

        assert_eq!(input.as_deref(), Some("video.mkv"));
        assert!(!menu_called.get());
        assert!(!prompt_called.get());
    }

    #[test]
    fn no_argument_prompts_only_after_menu_selection() {
        let input = resolve_input(None, || true, || Some("video.mkv".to_string()));
        assert_eq!(input.as_deref(), Some("video.mkv"));
    }

    #[test]
    fn leaving_menu_does_not_prompt_for_a_path() {
        let prompt_called = Cell::new(false);
        let input = resolve_input(
            None,
            || false,
            || {
                prompt_called.set(true);
                Some("video.mkv".to_string())
            },
        );

        assert!(input.is_none());
        assert!(!prompt_called.get());
    }
}
