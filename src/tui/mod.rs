use crate::{interfaces::probe::Subtitle, localization, settings};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend, layout::Rect, widgets::Clear};
use rust_i18n::t;
use std::{io, time::Duration};

type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

mod file_picker;
mod language;
mod prompts;
mod screens;

use file_picker::*;
use language::*;
use prompts::*;
use screens::*;
pub use screens::{MenuChoice, MessageKind};

fn draw_main_menu_overlay(
    frame: &mut Frame<'_>,
    background_selection: usize,
    locale: &str,
    width_percent: u16,
    height_percent: u16,
) -> Rect {
    draw_main_menu(
        frame,
        background_selection,
        &main_menu_options(locale),
        locale,
    );
    let area = centered_rect(frame.area(), width_percent, height_percent);
    frame.render_widget(Clear, area);
    area
}

pub struct TuiSession {
    terminal: TuiTerminal,
    selected_menu_item: usize,
}

impl TuiSession {
    pub fn new() -> Self {
        Self {
            terminal: start_terminal(),
            selected_menu_item: 0,
        }
    }

    pub fn main_menu(&mut self) -> MenuChoice {
        let terminal = &mut self.terminal;
        let mut selected = self.selected_menu_item;

        'menu: loop {
            let locale = rust_i18n::locale();
            let options = main_menu_options(locale.as_ref());
            require_ui(terminal.draw(|frame| {
                draw_main_menu(frame, selected, &options, locale.as_ref());
            }));

            if let Event::Key(key) = require_ui(event::read()) {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Up => {
                        selected = previous_selection(selected);
                        self.selected_menu_item = selected;
                    }
                    KeyCode::Down => {
                        selected = next_selection(selected, options.len());
                        self.selected_menu_item = selected;
                    }
                    KeyCode::Esc => {
                        self.selected_menu_item = selected;
                        break 'menu MenuChoice::Exit;
                    }
                    KeyCode::Enter => match menu_action(selected) {
                        MenuAction::Extract => {
                            self.selected_menu_item = selected;
                            break 'menu MenuChoice::Extract;
                        }
                        MenuAction::Translate => {
                            self.selected_menu_item = selected;
                            break 'menu MenuChoice::Translate;
                        }
                        MenuAction::ExtractAndTranslate => {
                            self.selected_menu_item = selected;
                            break 'menu MenuChoice::ExtractAndTranslate;
                        }
                        MenuAction::Transcribe => {
                            self.selected_menu_item = selected;
                            break 'menu MenuChoice::Transcribe;
                        }
                        MenuAction::TranscribeAndTranslate => {
                            self.selected_menu_item = selected;
                            break 'menu MenuChoice::TranscribeAndTranslate;
                        }
                        MenuAction::Options => {
                            self.selected_menu_item = selected;
                            settings_menu(terminal, selected);
                        }
                        MenuAction::Exit => {
                            self.selected_menu_item = selected;
                            break 'menu MenuChoice::Exit;
                        }
                        MenuAction::Noop => continue,
                    },
                    _ => {}
                }
            }
        }
    }

    pub fn show_message(&mut self, kind: MessageKind, body: &str) {
        drain_pending_events();

        let max_scroll = u16::try_from(body.lines().count().saturating_sub(1)).unwrap_or(u16::MAX);
        let mut scroll = 0u16;

        loop {
            let locale = rust_i18n::locale();
            require_ui(self.terminal.draw(|frame| {
                let area =
                    draw_main_menu_overlay(frame, self.selected_menu_item, locale.as_ref(), 80, 75);
                draw_message(frame, area, kind, body, scroll, locale.as_ref());
            }));

            if let Event::Key(key) = require_ui(event::read()) {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Up => scroll = scroll.saturating_sub(1),
                    KeyCode::Down => scroll = (scroll + 1).min(max_scroll),
                    KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => break,
                    _ => {}
                }
            }
        }
    }

    pub fn with_status<T>(&mut self, body: &str, work: impl FnOnce() -> T) -> T {
        let locale = rust_i18n::locale();
        require_ui(self.terminal.draw(|frame| {
            let area =
                draw_main_menu_overlay(frame, self.selected_menu_item, locale.as_ref(), 65, 40);
            draw_status(frame, area, body, locale.as_ref());
        }));
        let result = work();
        drain_pending_events();
        result
    }

    pub fn show_progress(&mut self, body: &str, percent: u8) {
        let locale = rust_i18n::locale();
        require_ui(self.terminal.draw(|frame| {
            let area =
                draw_main_menu_overlay(frame, self.selected_menu_item, locale.as_ref(), 70, 45);
            draw_progress(frame, area, body, percent, locale.as_ref());
        }));
    }

    pub fn select_subtitle(&mut self, subtitle_streams: &[Subtitle]) -> Option<usize> {
        select_subtitle_in_terminal(
            &mut self.terminal,
            subtitle_streams,
            self.selected_menu_item,
        )
    }

    pub fn prompt_input_path(&mut self) -> Option<String> {
        self.prompt_file(FilePickerFilter::Video)
    }

    pub fn prompt_output_name(&mut self, input: &str) -> String {
        prompt_output_name_in_terminal(&mut self.terminal, input, self.selected_menu_item)
    }

    pub fn prompt_srt_path(&mut self) -> Option<String> {
        self.prompt_file(FilePickerFilter::Subtitle)
    }

    pub fn prompt_source_language(&mut self) -> Option<String> {
        let locale = rust_i18n::locale().to_string();
        let labels = LanguageSelectorLabels {
            search: &t!("translate.source_search", locale = locale),
            field: &t!("translate.source_field", locale = locale),
            help: &t!("translate.source_select_help", locale = locale),
            auto_option: Some(&t!("translate.source_auto", locale = locale)),
        };
        language_selector(
            &mut self.terminal,
            crate::languages::SupportedLanguage::ALL,
            &locale,
            &labels,
            self.selected_menu_item,
        )
        .map(|language| normalize_language(&language))
    }

    pub fn prompt_target_language(&mut self) -> Option<String> {
        let locale = rust_i18n::locale().to_string();
        target_language_selector(
            &mut self.terminal,
            crate::languages::SupportedLanguage::ALL,
            &locale,
            self.selected_menu_item,
        )
    }

    pub fn prompt_whisper_language(
        &mut self,
        languages: &[crate::languages::SupportedLanguage],
    ) -> Option<String> {
        let locale = rust_i18n::locale().to_string();
        let labels = LanguageSelectorLabels {
            search: &t!("whisper.language_search"),
            field: &t!("whisper.language_field"),
            help: &t!("whisper.language_help"),
            auto_option: None,
        };
        language_selector(
            &mut self.terminal,
            languages,
            &locale,
            &labels,
            self.selected_menu_item,
        )
    }

    pub fn prompt_translation_output_name(&mut self, input: &str, target: &str) -> Option<String> {
        let default = default_translation_name(input, target);
        prompt_line_in_terminal(
            &mut self.terminal,
            &t!("translate.output_prompt"),
            &t!("tui.output_file"),
            Some(&default),
            false,
            self.selected_menu_item,
        )
        .map(|name| finalize_output_name(&default, &name))
    }

    fn prompt_file(&mut self, filter: FilePickerFilter) -> Option<String> {
        match file_picker_in_terminal(&mut self.terminal, filter, self.selected_menu_item) {
            Ok(path) => path,
            Err(error) => {
                self.show_message(MessageKind::Error, &error);
                None
            }
        }
    }
}

impl Drop for TuiSession {
    fn drop(&mut self) {
        restore_terminal(&mut self.terminal);
    }
}

fn require_ui<T, E>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("{}", t!("errors.tui_failure")))
}

fn drain_pending_events() {
    while event::poll(Duration::ZERO).unwrap_or(false) {
        if event::read().is_err() {
            break;
        }
    }
}

fn start_terminal() -> TuiTerminal {
    require_ui(enable_raw_mode());
    let mut stdout = io::stdout();
    require_ui(execute!(stdout, EnterAlternateScreen, cursor::Hide));

    let backend = CrosstermBackend::new(stdout);
    require_ui(Terminal::new(backend))
}

fn restore_terminal(terminal: &mut TuiTerminal) {
    require_ui(disable_raw_mode());
    require_ui(execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        cursor::Show
    ));
}

fn settings_menu(terminal: &mut TuiTerminal, background_selection: usize) {
    let mut selected = 0usize;
    let mut status: Option<String> = None;

    loop {
        let locale = rust_i18n::locale();
        let options = settings_options(
            locale.as_ref(),
            settings::has_configured_api_key(),
            settings::api_key_enabled(),
        );
        require_ui(terminal.draw(|frame| {
            let area = draw_main_menu_overlay(frame, background_selection, locale.as_ref(), 65, 60);
            draw_settings_menu(
                frame,
                area,
                selected,
                &options,
                status.as_deref(),
                locale.as_ref(),
            );
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, options.len()),
                KeyCode::Esc => break,
                KeyCode::Enter => match settings_action(selected) {
                    SettingsAction::Language => {
                        language_menu(terminal, background_selection);
                        status = None;
                    }
                    SettingsAction::SetApiKey => {
                        if let Some(api_key) = prompt_api_key(terminal, background_selection) {
                            status = match settings::save_api_key(&api_key) {
                                Ok(()) => Some(t!("settings.api_key_saved").to_string()),
                                Err(error) => {
                                    Some(t!("settings.save_failed", error = error).to_string())
                                }
                            };
                        }
                    }
                    SettingsAction::ToggleApiKey => {
                        let enabled = !settings::api_key_enabled();
                        status = match settings::set_api_key_enabled(enabled) {
                            Ok(()) => Some(if enabled {
                                t!("settings.use_key_enabled").to_string()
                            } else {
                                t!("settings.use_key_disabled").to_string()
                            }),
                            Err(error) => {
                                Some(t!("settings.save_failed", error = error).to_string())
                            }
                        };
                    }
                    SettingsAction::RemoveApiKey => {
                        status = match settings::clear_saved_api_key() {
                            Ok(()) => Some(t!("settings.api_key_removed").to_string()),
                            Err(error) => {
                                Some(t!("settings.save_failed", error = error).to_string())
                            }
                        };
                    }
                    SettingsAction::Back => break,
                },
                _ => {}
            }
        }
    }
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let width = area.width.saturating_mul(width_percent) / 100;
    let height = area.height.saturating_mul(height_percent) / 100;

    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

fn language_menu(terminal: &mut TuiTerminal, background_selection: usize) {
    let locales = ["pt", "en", "es"];
    let mut selected = match rust_i18n::locale().as_ref() {
        "pt" => 0,
        "en" => 1,
        "es" => 2,
        _ => 1,
    };

    loop {
        let locale = rust_i18n::locale();
        require_ui(terminal.draw(|frame| {
            let area = draw_main_menu_overlay(frame, background_selection, locale.as_ref(), 50, 45);
            draw_language_menu(frame, area, selected, locale.as_ref());
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, locales.len()),
                KeyCode::Enter => {
                    localization::set_language(locales[selected]);
                    break;
                }
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }
}

fn select_subtitle_in_terminal(
    terminal: &mut TuiTerminal,
    subtitle_streams: &[Subtitle],
    background_selection: usize,
) -> Option<usize> {
    let mut selected = 0usize;

    loop {
        let locale = rust_i18n::locale();
        require_ui(terminal.draw(|frame| {
            draw_subtitle_selection(
                frame,
                subtitle_streams,
                selected,
                locale.as_ref(),
                background_selection,
            );
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break None,
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, subtitle_streams.len()),
                KeyCode::Enter => break Some(selected),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
