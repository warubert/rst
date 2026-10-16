use crate::{interfaces::probe::Subtitle, localization, settings};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use rust_i18n::t;
use std::{io, time::Duration};

type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub struct TuiSession {
    terminal: TuiTerminal,
}

impl TuiSession {
    pub fn new() -> Self {
        Self {
            terminal: start_terminal(),
        }
    }

    pub fn show_message(&mut self, kind: MessageKind, body: &str) {
        drain_pending_events();

        let max_scroll = u16::try_from(body.lines().count().saturating_sub(1)).unwrap_or(u16::MAX);
        let mut scroll = 0u16;

        loop {
            let locale = rust_i18n::locale();
            require_ui(self.terminal.draw(|frame| {
                draw_message(frame, kind, body, scroll, locale.as_ref());
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
        require_ui(
            self.terminal
                .draw(|frame| draw_status(frame, body, locale.as_ref())),
        );
        let result = work();
        drain_pending_events();
        result
    }

    pub fn select_subtitle(&mut self, subtitle_streams: &[Subtitle]) -> Option<usize> {
        select_subtitle_in_terminal(&mut self.terminal, subtitle_streams)
    }

    pub fn prompt_input_path(&mut self) -> Option<String> {
        prompt_input_path_in_terminal(&mut self.terminal)
    }

    pub fn prompt_output_name(&mut self, input: &str) -> String {
        prompt_output_name_in_terminal(&mut self.terminal, input)
    }

    pub fn prompt_srt_path(&mut self) -> Option<String> {
        prompt_line_in_terminal(
            &mut self.terminal,
            &t!("translate.srt_path_prompt"),
            &t!("translate.srt_path_field"),
            None,
            false,
        )
    }

    pub fn prompt_source_language(&mut self) -> Option<String> {
        prompt_line_in_terminal(
            &mut self.terminal,
            &t!("translate.source_prompt"),
            &t!("translate.source_field"),
            Some("auto"),
            false,
        )
        .map(|language| normalize_language(&language))
    }

    pub fn prompt_target_language(&mut self) -> Option<String> {
        let locale = rust_i18n::locale().to_string();
        target_language_selector(
            &mut self.terminal,
            crate::languages::SupportedLanguage::ALL,
            &locale,
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
        )
        .map(|name| finalize_output_name(&default, &name))
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
#[derive(Debug, PartialEq, Eq)]
enum MenuAction {
    Extract,
    Translate,
    ExtractAndTranslate,
    Noop,
    Options,
    Exit,
}

#[derive(Debug, PartialEq, Eq)]
enum SettingsAction {
    Language,
    SetApiKey,
    ToggleApiKey,
    RemoveApiKey,
    Back,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Extract,
    Translate,
    ExtractAndTranslate,
    Exit,
}

fn menu_action(selected: usize) -> MenuAction {
    match selected {
        0 => MenuAction::Extract,
        1 => MenuAction::Translate,
        2 => MenuAction::ExtractAndTranslate,
        3 => MenuAction::Options,
        4 => MenuAction::Exit,
        _ => MenuAction::Noop,
    }
}

fn settings_action(selected: usize) -> SettingsAction {
    match selected {
        0 => SettingsAction::Language,
        1 => SettingsAction::SetApiKey,
        2 => SettingsAction::ToggleApiKey,
        3 => SettingsAction::RemoveApiKey,
        _ => SettingsAction::Back,
    }
}

fn settings_options(locale: &str, has_key: bool, use_key: bool) -> [String; 5] {
    [
        t!("settings.language", locale = locale).to_string(),
        if has_key {
            t!("settings.api_key_set", locale = locale).to_string()
        } else {
            t!("settings.api_key_unset", locale = locale).to_string()
        },
        if use_key {
            t!("settings.use_key_on", locale = locale).to_string()
        } else {
            t!("settings.use_key_off", locale = locale).to_string()
        },
        t!("settings.remove_api_key", locale = locale).to_string(),
        t!("settings.back", locale = locale).to_string(),
    ]
}

fn mask_input(value: &str) -> String {
    "*".repeat(value.chars().count())
}

fn previous_selection(selected: usize) -> usize {
    selected.saturating_sub(1)
}

fn next_selection(selected: usize, item_count: usize) -> usize {
    (selected + 1).min(item_count.saturating_sub(1))
}

fn main_menu_options(locale: &str) -> [String; 5] {
    [
        t!("menu.extract", locale = locale).to_string(),
        t!("menu.translate", locale = locale).to_string(),
        t!("menu.extract_translate", locale = locale).to_string(),
        t!("menu.options", locale = locale).to_string(),
        t!("menu.exit", locale = locale).to_string(),
    ]
}

fn draw_main_menu(frame: &mut Frame<'_>, selected: usize, options: &[String], locale: &str) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(1),
            Constraint::Min(7),
            Constraint::Length(3),
        ])
        .split(area);

    let logo_rows = [
        ("###### ", " ######", "###########"),
        ("##   ##", "##     ", "    ###    "),
        ("##   ##", "##     ", "    ###    "),
        ("##   ##", " ######", "    ###    "),
        ("###### ", "     ##", "    ###    "),
        ("## ##  ", "     ##", "    ###    "),
        ("##  ## ", "     ##", "    ###    "),
        ("##   ##", "##   ##", "    ###    "),
        ("##   ##", " ######", "    ###    "),
    ];
    let logo = logo_rows
        .iter()
        .map(|(r, s, t)| format!("{r}   {s}   {t}"))
        .collect::<Vec<_>>()
        .join("\n");
    let logo = Paragraph::new(logo)
        .style(
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(logo, chunks[0]);

    let tagline = Paragraph::new("Rust Subtitle Tools")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(tagline, chunks[1]);

    let items: Vec<ListItem> = options
        .iter()
        .map(|option| ListItem::new(option.clone()))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    let menu = List::new(items)
        .block(
            Block::default()
                .title(t!("menu.title", locale = locale).to_string())
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(menu, chunks[2], &mut list_state);

    let help = Paragraph::new(t!("menu.help", locale = locale).to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("menu.actions", locale = locale).to_string()),
        );
    frame.render_widget(help, chunks[3]);
}

fn draw_language_menu(frame: &mut Frame<'_>, selected: usize, locale: &str) {
    let options = [
        t!("language.pt", locale = locale).to_string(),
        t!("language.en", locale = locale).to_string(),
        t!("language.es", locale = locale).to_string(),
    ];
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    let items: Vec<ListItem> = options
        .iter()
        .map(|option| ListItem::new(option.clone()))
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));

    let languages = List::new(items)
        .block(
            Block::default()
                .title(t!("language.title", locale = locale).to_string())
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(languages, chunks[0], &mut state);

    let help = Paragraph::new(t!("language.help", locale = locale).to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("menu.actions", locale = locale).to_string()),
        );
    frame.render_widget(help, chunks[1]);
}

fn matching_language_indices(
    languages: &[crate::languages::SupportedLanguage],
    query: &str,
    locale: &str,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    languages
        .iter()
        .enumerate()
        .filter_map(|(index, language)| {
            (query.is_empty()
                || language.name_for(locale).to_lowercase().contains(&query)
                || language.code().to_lowercase().contains(&query))
            .then_some(index)
        })
        .collect()
}

fn selected_target_language_index(
    languages: &[crate::languages::SupportedLanguage],
    default: &str,
) -> usize {
    languages
        .iter()
        .position(|language| language.code().eq_ignore_ascii_case(default))
        .unwrap_or(0)
}

fn draw_target_language_selector(
    frame: &mut Frame<'_>,
    languages: &[crate::languages::SupportedLanguage],
    query: &str,
    selected: usize,
    locale: &str,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let search = Paragraph::new(query.to_string())
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("translate.target_search", locale = locale).to_string()),
        );
    frame.render_widget(search, chunks[0]);

    let filtered = matching_language_indices(languages, query, locale);
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|&index| {
            let language = &languages[index];
            ListItem::new(format!(
                "{} ({})",
                language.name_for(locale),
                language.code()
            ))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("translate.target_field", locale = locale).to_string()),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new(t!("translate.target_select_help", locale = locale).to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("menu.actions", locale = locale).to_string()),
        );
    frame.render_widget(help, chunks[2]);
}

fn target_language_selector(
    terminal: &mut TuiTerminal,
    languages: &[crate::languages::SupportedLanguage],
    locale: &str,
) -> Option<String> {
    let mut query = String::new();
    let default = normalize_language(locale);
    let mut selected = selected_target_language_index(languages, &default);

    loop {
        let filtered = matching_language_indices(languages, &query, locale);
        if !filtered.is_empty() {
            selected = selected.min(filtered.len() - 1);
        } else {
            selected = 0;
        }

        require_ui(terminal.draw(|frame| {
            draw_target_language_selector(frame, languages, &query, selected, locale);
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char(character) if !character.is_control() => {
                    query.push(character);
                    selected = 0;
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, filtered.len()),
                KeyCode::Enter => {
                    break filtered
                        .get(selected)
                        .map(|&index| languages[index].code().to_string());
                }
                KeyCode::Esc => break None,
                _ => {}
            }
        }
    }
}

fn draw_settings_menu(
    frame: &mut Frame<'_>,
    selected: usize,
    options: &[String],
    status: Option<&str>,
    locale: &str,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(7), Constraint::Length(3)])
        .split(area);

    let items: Vec<ListItem> = options
        .iter()
        .map(|option| ListItem::new(option.clone()))
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));

    let settings_list = List::new(items)
        .block(
            Block::default()
                .title(t!("settings.title", locale = locale).to_string())
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(settings_list, chunks[0], &mut state);

    let hint = status
        .map(str::to_string)
        .unwrap_or_else(|| t!("settings.help", locale = locale).to_string());
    let help = Paragraph::new(hint).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .title(t!("menu.actions", locale = locale).to_string()),
    );
    frame.render_widget(help, chunks[1]);
}

fn draw_subtitle_selection(
    frame: &mut Frame<'_>,
    subtitle_streams: &[Subtitle],
    selected: usize,
    locale: &str,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(area);

    let items: Vec<ListItem> = subtitle_streams
        .iter()
        .enumerate()
        .map(|(index, subtitle)| {
            let label = if subtitle.is_sdh {
                format!("{} - {} (SDH)", index + 1, subtitle.language)
            } else {
                format!("{} - {}", index + 1, subtitle.language)
            };
            ListItem::new(label)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    let subtitles = List::new(items)
        .block(
            Block::default()
                .title(t!("tui.subtitles", locale = locale).to_string())
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(subtitles, chunks[0], &mut list_state);

    let help = Paragraph::new(t!("tui.subtitle_help", locale = locale).to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("menu.actions", locale = locale).to_string()),
        );
    frame.render_widget(help, chunks[1]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Success,
    Error,
}

fn message_title(kind: MessageKind, locale: &str) -> (String, Color) {
    match kind {
        MessageKind::Info => (
            t!("messages.info_title", locale = locale).to_string(),
            Color::Yellow,
        ),
        MessageKind::Success => (
            t!("messages.success_title", locale = locale).to_string(),
            Color::Green,
        ),
        MessageKind::Error => (
            t!("messages.error_title", locale = locale).to_string(),
            Color::Red,
        ),
    }
}

fn draw_message(frame: &mut Frame<'_>, kind: MessageKind, body: &str, scroll: u16, locale: &str) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    let (title, color) = message_title(kind, locale);
    let message = Paragraph::new(body.to_string())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color))
                .title(title)
                .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        );
    frame.render_widget(message, chunks[0]);

    let hint = Paragraph::new(t!("messages.continue_hint", locale = locale).to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("menu.actions", locale = locale).to_string()),
        );
    frame.render_widget(hint, chunks[1]);
}

fn draw_status(frame: &mut Frame<'_>, body: &str, locale: &str) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(6)
        .constraints([Constraint::Min(5)])
        .split(area);

    let status = Paragraph::new(body.to_string())
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("messages.working_title", locale = locale).to_string())
                .title_alignment(Alignment::Center),
        );
    frame.render_widget(status, chunks[0]);
}

fn drain_pending_events() {
    while event::poll(Duration::ZERO).unwrap_or(false) {
        if event::read().is_err() {
            break;
        }
    }
}

pub fn show_message(kind: MessageKind, body: &str) {
    TuiSession::new().show_message(kind, body);
}

fn default_output_name(input: &str) -> String {
    let path = std::path::Path::new(input);
    if path.extension().is_some() {
        path.with_extension(String::new())
            .to_string_lossy()
            .into_owned()
    } else {
        input.to_string()
    }
}

fn finalize_output_name(default_name: &str, value: &str) -> String {
    let final_name = if value.trim().is_empty() {
        default_name.to_string()
    } else {
        value.to_string()
    };

    if final_name.to_lowercase().ends_with(".srt") {
        final_name
    } else {
        format!("{final_name}.srt")
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

pub fn main_menu() -> MenuChoice {
    let mut session = TuiSession::new();
    let terminal = &mut session.terminal;
    let mut selected = 0usize;

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
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, options.len()),
                KeyCode::Esc => break 'menu MenuChoice::Exit,
                KeyCode::Enter => match menu_action(selected) {
                    MenuAction::Extract => break 'menu MenuChoice::Extract,
                    MenuAction::Translate => break 'menu MenuChoice::Translate,
                    MenuAction::ExtractAndTranslate => {
                        break 'menu MenuChoice::ExtractAndTranslate;
                    }
                    MenuAction::Options => settings_menu(terminal),
                    MenuAction::Exit => break 'menu MenuChoice::Exit,
                    MenuAction::Noop => continue,
                },
                _ => {}
            }
        }
    }
}

fn settings_menu(terminal: &mut TuiTerminal) {
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
            draw_settings_menu(
                frame,
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
                        language_menu(terminal);
                        status = None;
                    }
                    SettingsAction::SetApiKey => {
                        if let Some(api_key) = prompt_api_key(terminal) {
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

fn prompt_api_key(terminal: &mut TuiTerminal) -> Option<String> {
    prompt_line_in_terminal(
        terminal,
        &t!("settings.api_key_prompt"),
        &t!("settings.api_key_field"),
        None,
        true,
    )
}

fn draw_prompt(frame: &mut Frame<'_>, title: &str, field_title: &str, display: &str) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(6)
        .constraints([Constraint::Length(4), Constraint::Length(3)])
        .split(area);

    let title = Paragraph::new(title.to_string())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("RST"));
    frame.render_widget(title, chunks[0]);

    let field = Paragraph::new(display.to_string())
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(field_title.to_string())
                .title_alignment(Alignment::Center),
        );
    frame.render_widget(field, chunks[1]);
}

fn resolve_prompt_value(value: &str, default: Option<&str>) -> Option<String> {
    let value = value.trim();
    if !value.is_empty() {
        Some(value.to_string())
    } else {
        default.map(str::to_string)
    }
}

fn prompt_line_in_terminal(
    terminal: &mut TuiTerminal,
    title: &str,
    field_title: &str,
    default: Option<&str>,
    masked: bool,
) -> Option<String> {
    let mut value = String::new();

    loop {
        let display = match (value.is_empty(), default) {
            (true, Some(default)) => t!("tui.output_default", name = default).to_string(),
            _ => value.clone(),
        };
        let display = if masked {
            mask_input(&display)
        } else {
            display
        };
        require_ui(terminal.draw(|frame| draw_prompt(frame, title, field_title, &display)));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char(c) if !c.is_control() => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Esc => break None,
                KeyCode::Enter => {
                    if let Some(resolved) = resolve_prompt_value(&value, default) {
                        break Some(resolved);
                    }
                }
                _ => {}
            }
        }
    }
}

fn normalize_language(language: &str) -> String {
    language.trim().to_ascii_lowercase()
}

fn default_translation_name(input: &str, target: &str) -> String {
    format!("{}-{}", default_output_name(input), target)
}

fn language_menu(terminal: &mut TuiTerminal) {
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
            draw_language_menu(frame, selected, locale.as_ref());
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

pub fn prompt_input_path() -> Option<String> {
    TuiSession::new().prompt_input_path()
}

fn prompt_input_path_in_terminal(terminal: &mut TuiTerminal) -> Option<String> {
    let mut value = String::new();

    loop {
        require_ui(terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(6)
                .constraints([Constraint::Length(4), Constraint::Length(3)])
                .split(area);

            let title = Paragraph::new(t!("tui.input_prompt").to_string())
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title("RST"));
            frame.render_widget(title, chunks[0]);

            let field = Paragraph::new(value.as_str())
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(t!("tui.input_path").to_string())
                        .title_alignment(Alignment::Center),
                );
            frame.render_widget(field, chunks[1]);
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char(c) if !c.is_control() => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Esc => {
                    return None;
                }
                KeyCode::Enter if !value.trim().is_empty() => break,
                _ => {}
            }
        }
    }

    Some(value.trim().to_string())
}

fn prompt_output_name_in_terminal(terminal: &mut TuiTerminal, input: &str) -> String {
    let default_output_name = default_output_name(input);
    let mut value = String::new();

    loop {
        require_ui(terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(6)
                .constraints([Constraint::Length(4), Constraint::Length(3)])
                .split(area);

            let title = Paragraph::new(t!("tui.output_prompt").to_string())
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title("RST"));
            frame.render_widget(title, chunks[0]);

            let display = if value.is_empty() {
                t!("tui.output_default", name = default_output_name).to_string()
            } else {
                value.clone()
            };

            let field = Paragraph::new(display)
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(t!("tui.output_file").to_string())
                        .title_alignment(Alignment::Center),
                );
            frame.render_widget(field, chunks[1]);
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char(c) if !c.is_control() => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Esc => {
                    value.clear();
                    break;
                }
                KeyCode::Enter => break,
                _ => {}
            }
        }
    }

    finalize_output_name(&default_output_name, &value)
}

fn select_subtitle_in_terminal(
    terminal: &mut TuiTerminal,
    subtitle_streams: &[Subtitle],
) -> Option<usize> {
    let mut selected = 0usize;

    loop {
        let locale = rust_i18n::locale();
        require_ui(terminal.draw(|frame| {
            draw_subtitle_selection(frame, subtitle_streams, selected, locale.as_ref());
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
mod tests {
    use super::{
        MenuAction, MessageKind, SettingsAction, default_output_name, default_translation_name,
        draw_language_menu, draw_main_menu, draw_message, draw_prompt, draw_settings_menu,
        draw_status, draw_subtitle_selection, draw_target_language_selector, finalize_output_name,
        main_menu_options, mask_input, matching_language_indices, menu_action, next_selection,
        normalize_language, previous_selection, resolve_prompt_value,
        selected_target_language_index, settings_action, settings_options,
    };
    use crate::{interfaces::probe::Subtitle, languages::SupportedLanguage};
    use ratatui::{Terminal, backend::TestBackend};

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
        assert_eq!(menu_action(3), MenuAction::Options);
        assert_eq!(menu_action(4), MenuAction::Exit);
        assert_eq!(menu_action(5), MenuAction::Noop);
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
            .draw(|frame| draw_settings_menu(frame, 1, &options, None, "en"))
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
            .draw(|frame| draw_prompt(frame, "Target language", "Language", "pt (default)"))
            .unwrap();

        let text = rendered_text(&terminal);
        assert!(text.contains("Target language"));
        assert!(text.contains("Language"));
        assert!(text.contains("pt (default)"));
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
    fn renders_target_language_search_and_matching_choices() {
        let languages = [SupportedLanguage::LangEn, SupportedLanguage::LangPt];
        let backend = TestBackend::new(80, 18);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| draw_target_language_selector(frame, &languages, "port", 0, "pt"))
            .unwrap();

        let text = rendered_text(&terminal);
        assert!(text.contains("Filtrar por nome ou código"));
        assert!(text.contains("Português (Brasil) (pt)"));
        assert!(!text.contains("Inglês (en)"));
    }

    #[test]
    fn renders_api_key_input_only_as_asterisks() {
        let secret = "example-private-key";
        let masked = mask_input(secret);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| draw_prompt(frame, "API key", "Google Translate", &masked))
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
                .draw(|frame| draw_message(frame, kind, "Something happened", 0, "en"))
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
            .draw(|frame| draw_message(frame, MessageKind::Error, body, 1, "en"))
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
            .draw(|frame| draw_message(frame, MessageKind::Success, "ok", 0, "pt"))
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
            .draw(|frame| draw_status(frame, "Translating episode.srt", "en"))
            .unwrap();

        let text = rendered_text(&terminal);
        assert!(text.contains("Working"));
        assert!(text.contains("Translating episode.srt"));
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
        assert!(text.contains("5) Exit"));
    }

    #[test]
    fn renders_language_options_with_ratatui() {
        let backend = TestBackend::new(80, 18);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| draw_language_menu(frame, 0, "pt"))
            .unwrap();

        let text = rendered_text(&terminal);
        assert!(text.contains("Idioma"));
        assert!(text.contains("Português"));
        assert!(text.contains("English"));
        assert!(text.contains("Español"));
    }

    #[test]
    fn renders_subtitle_languages_and_sdh_marker() {
        let backend = TestBackend::new(80, 20);
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
            .draw(|frame| draw_subtitle_selection(frame, &subtitles, 0, "pt"))
            .unwrap();

        let text = rendered_text(&terminal);
        assert!(text.contains("Legendas disponíveis"));
        assert!(text.contains("1 - pt-BR (SDH)"));
        assert!(text.contains("2 - en"));
    }
}
