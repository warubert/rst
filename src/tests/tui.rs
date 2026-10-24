use super::{
    FilePickerEntry, FilePickerFilter, FilePickerView, LanguageSelectorLabels, MenuAction,
    MessageKind, SettingsAction, default_output_name, default_translation_name, draw_file_picker,
    draw_language_menu, draw_language_selector, draw_main_menu, draw_message, draw_modal_prompt,
    draw_progress, draw_prompt_in_area, draw_settings_menu, draw_status, draw_subtitle_selection,
    file_picker_entry_style, filter_file_picker_entries, finalize_output_name,
    initial_language_selection, language_selector_code, main_menu_options, mask_input,
    matches_file_picker_filter, matching_language_indices, matching_language_selector_indices,
    menu_action, next_selection, normalize_language, previous_selection, read_file_picker_entries,
    resolve_prompt_value, selected_target_language_index, settings_action, settings_options,
};
use crate::{interfaces::probe::Subtitle, languages::SupportedLanguage};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn maps_main_menu_options_to_their_actions() {
    assert_eq!(menu_action(0), MenuAction::Extract);
    assert_eq!(menu_action(1), MenuAction::Translate);
    assert_eq!(menu_action(2), MenuAction::ExtractAndTranslate);
    assert_eq!(menu_action(3), MenuAction::Transcribe);
    assert_eq!(menu_action(4), MenuAction::TranscribeAndTranslate);
    assert_eq!(menu_action(5), MenuAction::Options);
    assert_eq!(menu_action(6), MenuAction::Exit);
    assert_eq!(menu_action(7), MenuAction::Noop);
}

#[test]
fn derives_default_name_without_video_extension() {
    assert_eq!(default_output_name("episode.mkv"), "episode");
    assert_eq!(default_output_name("episode"), "episode");
    assert_eq!(
        default_output_name("folder.with.dots/episode"),
        "folder.with.dots/episode"
    );
    assert_eq!(
        default_output_name("folder.with.dots/episode.mkv"),
        "folder.with.dots/episode"
    );
}

#[test]
fn adds_srt_extension_and_preserves_existing_extension_case() {
    assert_eq!(finalize_output_name("episode", "captions"), "captions.srt");
    assert_eq!(
        finalize_output_name("episode", "captions.SRT"),
        "captions.SRT"
    );
}

#[test]
fn blank_output_name_uses_video_name_as_default() {
    assert_eq!(finalize_output_name("episode", "  "), "episode.srt");
}

#[test]
fn suggests_translation_name_from_input_and_target_language() {
    let default = default_translation_name("AloneS13E13.srt", "pt");
    assert_eq!(default, "AloneS13E13-pt");
    assert_eq!(finalize_output_name(&default, ""), "AloneS13E13-pt.srt");
    assert_eq!(
        default_translation_name("dir.v2/Alone.srt", "es"),
        "dir.v2/Alone-es"
    );
}

#[test]
fn prompt_value_falls_back_to_default_only_when_blank() {
    assert_eq!(
        resolve_prompt_value("  en ", Some("pt")).as_deref(),
        Some("en")
    );
    assert_eq!(resolve_prompt_value("", Some("pt")).as_deref(), Some("pt"));
    assert_eq!(resolve_prompt_value("   ", None), None);
}

#[test]
fn file_picker_filters_supported_extensions_without_hiding_folders() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("rst-file-picker-{timestamp}"));
    fs::create_dir_all(directory.join("clips")).unwrap();
    fs::write(directory.join("episode.MKV"), "").unwrap();
    fs::write(directory.join("captions.srt"), "").unwrap();
    fs::write(directory.join("notes.txt"), "").unwrap();

    let videos = read_file_picker_entries(&directory, FilePickerFilter::Video).unwrap();
    assert_eq!(videos.len(), 3);
    assert_eq!(videos[0], FilePickerEntry::Parent);
    assert_eq!(
        videos[1],
        FilePickerEntry::Directory(directory.join("clips"))
    );
    assert_eq!(
        videos[2],
        FilePickerEntry::File(directory.join("episode.MKV"))
    );

    let subtitles = read_file_picker_entries(&directory, FilePickerFilter::Subtitle).unwrap();
    assert_eq!(subtitles.len(), 3);
    assert_eq!(subtitles[0], FilePickerEntry::Parent);
    assert_eq!(
        subtitles[1],
        FilePickerEntry::Directory(directory.join("clips"))
    );
    assert_eq!(
        subtitles[2],
        FilePickerEntry::File(directory.join("captions.srt"))
    );

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn file_picker_extension_filter_is_case_insensitive() {
    assert!(matches_file_picker_filter(
        std::path::Path::new("video.MOV"),
        FilePickerFilter::Video
    ));
    assert!(matches_file_picker_filter(
        std::path::Path::new("subtitles.SRT"),
        FilePickerFilter::Subtitle
    ));
    assert!(!matches_file_picker_filter(
        std::path::Path::new("readme.txt"),
        FilePickerFilter::Video
    ));
}

#[test]
fn file_picker_marks_supported_extensions_green_for_the_active_filter() {
    assert_eq!(
        file_picker_entry_style(
            &FilePickerEntry::File("movie.MKV".into()),
            FilePickerFilter::Video
        )
        .fg,
        Some(Color::LightGreen)
    );
    assert_eq!(
        file_picker_entry_style(
            &FilePickerEntry::File("captions.SRT".into()),
            FilePickerFilter::Subtitle
        )
        .fg,
        Some(Color::LightGreen)
    );
    assert_eq!(
        file_picker_entry_style(
            &FilePickerEntry::File("captions.srt".into()),
            FilePickerFilter::Video
        )
        .fg,
        None
    );
    assert_eq!(
        file_picker_entry_style(
            &FilePickerEntry::Directory("Movies".into()),
            FilePickerFilter::Video
        )
        .fg,
        None
    );
}

#[test]
fn file_picker_search_matches_names_case_insensitively_and_preserves_parent_when_clear() {
    let entries = [
        FilePickerEntry::Parent,
        FilePickerEntry::Directory("Movies".into()),
        FilePickerEntry::File("Episode.MKV".into()),
        FilePickerEntry::File("captions.srt".into()),
    ];

    assert_eq!(filter_file_picker_entries(&entries, ""), entries);
    assert_eq!(
        filter_file_picker_entries(&entries, "epi"),
        [FilePickerEntry::File("Episode.MKV".into())]
    );
    assert_eq!(
        filter_file_picker_entries(&entries, "movies"),
        [FilePickerEntry::Directory("Movies".into())]
    );
    assert!(filter_file_picker_entries(&entries, "no match").is_empty());
}

#[test]
fn renders_file_picker_with_location_selection_and_help() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).unwrap();
    let entries = [FilePickerEntry::File("episode.mkv".into())];

    terminal
        .draw(|frame| {
            draw_file_picker(
                frame,
                FilePickerView {
                    directory: std::path::Path::new("/videos"),
                    query: "epis",
                    entries: &entries,
                    selected: 0,
                    error: None,
                    filter: FilePickerFilter::Video,
                    background_selection: 0,
                    locale: "en",
                },
            )
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Select a video file"));
    assert!(text.contains("1) E"));
    assert!(text.contains("/videos"));
    assert!(text.contains("Filter files and folders"));
    assert!(text.contains("epis"));
    assert!(text.contains("episode.mkv"));
    assert!(text.contains("Backspace"));
    assert!(text.contains("F2"));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() == "e" && cell.fg == Color::LightGreen)
    );
}

#[test]
fn settings_menu_exposes_api_key_actions_and_toggle_state() {
    assert_eq!(settings_action(0), SettingsAction::Language);
    assert_eq!(settings_action(1), SettingsAction::SetApiKey);
    assert_eq!(settings_action(2), SettingsAction::ToggleApiKey);
    assert_eq!(settings_action(3), SettingsAction::RemoveApiKey);
    assert_eq!(settings_action(4), SettingsAction::Back);

    let enabled = settings_options("en", true, true);
    assert!(enabled[1].contains("***"), "{enabled:?}");
    assert!(enabled[2].contains("On"));
    assert!(!enabled.iter().any(|option| option.contains("real-secret")));

    let disabled = settings_options("en", false, false);
    assert!(disabled[1].contains("not set"));
    assert!(disabled[2].contains("Off"));
}

#[test]
fn api_key_input_is_masked_without_exposing_its_value() {
    assert_eq!(mask_input("secret-123"), "**********");
    assert_eq!(mask_input(""), "");
}

#[test]
fn renders_settings_without_exposing_api_key() {
    let backend = TestBackend::new(80, 22);
    let mut terminal = Terminal::new(backend).unwrap();
    let options = settings_options("en", true, true);

    terminal
        .draw(|frame| draw_settings_menu(frame, frame.area(), 1, &options, None, "en"))
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Options"), "{text}");
    assert!(text.contains("***"));
    assert!(text.contains("Use API key: On"));
    assert!(!text.contains("real-secret"));
}

#[test]
fn normalizes_language_codes() {
    assert_eq!(normalize_language(" PT-br "), "pt-br");
    assert_eq!(normalize_language("Auto"), "auto");
}

#[test]
fn renders_prompt_title_field_and_value() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            draw_prompt_in_area(
                frame,
                frame.area(),
                "Target language",
                "Language",
                "pt (default)",
                6,
            )
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Target language"));
    assert!(text.contains("Language"));
    assert!(text.contains("pt (default)"));
}

#[test]
fn renders_subtitle_name_prompt_over_the_main_menu() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            draw_modal_prompt(
                frame,
                "Output subtitle name",
                "File .srt",
                "episode.srt (default)",
                1,
                "en",
            )
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Output subtitle name"));
    assert!(text.contains("episode.srt (default)"));
    assert!(text.contains("2) T"));
}

#[test]
fn filters_target_languages_by_localized_name_or_code() {
    let languages = [
        SupportedLanguage::LangEn,
        SupportedLanguage::LangPt,
        SupportedLanguage::LangPtPt,
    ];

    assert_eq!(
        matching_language_indices(&languages, "portuguese", "en"),
        [1, 2]
    );
    assert_eq!(
        matching_language_indices(&languages, "português", "pt"),
        [1, 2]
    );
    assert_eq!(matching_language_indices(&languages, "inglés", "es"), [0]);
    assert_eq!(matching_language_indices(&languages, "PT-p", "en"), [2]);
    assert_eq!(matching_language_indices(&languages, "", "en"), [0, 1, 2]);
    assert!(matching_language_indices(&languages, "no-match", "en").is_empty());
}

#[test]
fn preselects_the_current_interface_locale_when_available() {
    let languages = [SupportedLanguage::LangEn, SupportedLanguage::LangPtPt];

    assert_eq!(selected_target_language_index(&languages, "pt-PT"), 1);
    assert_eq!(selected_target_language_index(&languages, "es"), 0);
}

#[test]
fn source_language_selector_includes_auto_and_defaults_to_it() {
    let languages = [SupportedLanguage::LangEn, SupportedLanguage::LangPt];

    assert_eq!(initial_language_selection(&languages, "pt", true), 0);
    assert_eq!(
        matching_language_selector_indices(&languages, "", "en", Some("Automatic")),
        [0, 1, 2]
    );
    assert_eq!(
        matching_language_selector_indices(&languages, "auto", "en", Some("Automatic")),
        [0]
    );
    assert_eq!(
        matching_language_selector_indices(&languages, "port", "en", Some("Automatic")),
        [2]
    );
    assert_eq!(language_selector_code(&languages, 0, true), "auto");
    assert_eq!(language_selector_code(&languages, 2, true), "pt");
}

#[test]
fn renders_automatic_as_the_first_source_language_option() {
    let languages = [SupportedLanguage::LangEn, SupportedLanguage::LangPt];
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            draw_language_selector(
                frame,
                &languages,
                "",
                0,
                "pt",
                &LanguageSelectorLabels {
                    search: "Filtrar idioma",
                    field: "Idioma original",
                    help: "Digite para filtrar",
                    auto_option: Some("Automático"),
                },
                0,
            )
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Automático (auto)"));
    assert!(text.contains("Português (Brasil) (pt)"));
}

#[test]
fn renders_target_language_search_and_matching_choices() {
    let languages = [SupportedLanguage::LangEn, SupportedLanguage::LangPt];
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            draw_language_selector(
                frame,
                &languages,
                "port",
                0,
                "pt",
                &LanguageSelectorLabels {
                    search: "Filtrar por nome ou código",
                    field: "Idioma de destino",
                    help: "Digite para filtrar",
                    auto_option: None,
                },
                1,
            )
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Filtrar por nome ou código"));
    assert!(text.contains("Português (Brasil) (pt)"));
    assert!(!text.contains("Inglês (en)"));
    assert!(text.contains("2) T"));
}

#[test]
fn renders_api_key_input_only_as_asterisks() {
    let secret = "example-private-key";
    let masked = mask_input(secret);
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            draw_prompt_in_area(
                frame,
                frame.area(),
                "API key",
                "Google Translate",
                &masked,
                6,
            )
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains(&"*".repeat(secret.len())));
    assert!(!text.contains(secret));
}

#[test]
fn selection_movement_stays_within_list_boundaries() {
    assert_eq!(previous_selection(0), 0);
    assert_eq!(previous_selection(2), 1);
    assert_eq!(next_selection(0, 5), 1);
    assert_eq!(next_selection(4, 5), 4);
    assert_eq!(next_selection(0, 0), 0);
}

#[test]
fn renders_each_message_kind_with_its_title_body_and_hint() {
    for (kind, title) in [
        (MessageKind::Info, "Notice"),
        (MessageKind::Success, "Success"),
        (MessageKind::Error, "Error"),
    ] {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                draw_message(frame, area, kind, "Something happened", 0, "en")
            })
            .unwrap();

        let text = rendered_text(&terminal);
        assert!(text.contains(title), "missing title for {kind:?}");
        assert!(text.contains("Something happened"));
        assert!(text.contains("to continue"));
    }
}

#[test]
fn scrolling_a_long_message_hides_the_first_lines() {
    let body = "first line\nsecond line\nthird line";
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = frame.area();
            draw_message(frame, area, MessageKind::Error, body, 1, "en")
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(!text.contains("first line"));
    assert!(text.contains("second line"));
    assert!(text.contains("third line"));
}

#[test]
fn renders_message_in_the_requested_locale() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = frame.area();
            draw_message(frame, area, MessageKind::Success, "ok", 0, "pt")
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Sucesso"));
    assert!(text.contains("para continuar"));
}

#[test]
fn renders_working_status() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = frame.area();
            draw_status(frame, area, "Translating episode.srt", "en")
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Working"));
    assert!(text.contains("Translating episode.srt"));
}

#[test]
fn renders_percentage_progress() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = frame.area();
            draw_progress(frame, area, "Transcribing video.mkv", 42, "en")
        })
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Transcribing video.mkv"));
    assert!(text.contains("42%"));
}

#[test]
fn renders_main_menu_in_requested_locale() {
    let backend = TestBackend::new(100, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    let options = main_menu_options("en");

    terminal
        .draw(|frame| draw_main_menu(frame, 0, &options, "en"))
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Main menu"));
    assert!(text.contains("1) Extract subtitles"));
    assert!(text.contains("4) Create subtitles with Whisper"));
    assert!(text.contains("5) Create subtitles with Whisper and translate"));
    assert!(text.contains("7) Exit"));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.fg == Color::LightCyan)
    );
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == Color::Cyan)
    );
}

#[test]
fn renders_language_options_with_ratatui() {
    let backend = TestBackend::new(80, 18);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw_language_menu(frame, frame.area(), 0, "pt"))
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Idioma"));
    assert!(text.contains("Português"));
    assert!(text.contains("English"));
    assert!(text.contains("Español"));
}

#[test]
fn renders_subtitle_languages_and_sdh_marker() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let subtitles = [
        Subtitle {
            index: 2,
            language: "pt-BR".to_string(),
            is_sdh: true,
        },
        Subtitle {
            index: 4,
            language: "en".to_string(),
            is_sdh: false,
        },
    ];

    terminal
        .draw(|frame| draw_subtitle_selection(frame, &subtitles, 0, "pt", 0))
        .unwrap();

    let text = rendered_text(&terminal);
    assert!(text.contains("Legendas disponíveis"));
    assert!(text.contains("1) Extra"));
    assert!(text.contains("1 - pt-BR (SDH)"));
    assert!(text.contains("2 - en"));
}
