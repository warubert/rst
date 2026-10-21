use super::draw_main_menu_overlay;
use crate::interfaces::probe::Subtitle;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};
use rust_i18n::t;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MenuAction {
    Extract,
    Translate,
    ExtractAndTranslate,
    Transcribe,
    TranscribeAndTranslate,
    Noop,
    Options,
    Exit,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SettingsAction {
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
    Transcribe,
    TranscribeAndTranslate,
    Exit,
}

pub(super) fn menu_action(selected: usize) -> MenuAction {
    match selected {
        0 => MenuAction::Extract,
        1 => MenuAction::Translate,
        2 => MenuAction::ExtractAndTranslate,
        3 => MenuAction::Transcribe,
        4 => MenuAction::TranscribeAndTranslate,
        5 => MenuAction::Options,
        6 => MenuAction::Exit,
        _ => MenuAction::Noop,
    }
}

pub(super) fn settings_action(selected: usize) -> SettingsAction {
    match selected {
        0 => SettingsAction::Language,
        1 => SettingsAction::SetApiKey,
        2 => SettingsAction::ToggleApiKey,
        3 => SettingsAction::RemoveApiKey,
        _ => SettingsAction::Back,
    }
}

pub(super) fn settings_options(locale: &str, has_key: bool, use_key: bool) -> [String; 5] {
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

pub(super) fn mask_input(value: &str) -> String {
    "*".repeat(value.chars().count())
}

pub(super) fn previous_selection(selected: usize) -> usize {
    selected.saturating_sub(1)
}

pub(super) fn next_selection(selected: usize, item_count: usize) -> usize {
    (selected + 1).min(item_count.saturating_sub(1))
}

pub(super) fn main_menu_options(locale: &str) -> [String; 7] {
    [
        t!("menu.extract", locale = locale).to_string(),
        t!("menu.translate", locale = locale).to_string(),
        t!("menu.extract_translate", locale = locale).to_string(),
        t!("menu.transcribe", locale = locale).to_string(),
        t!("menu.transcribe_translate", locale = locale).to_string(),
        t!("menu.options", locale = locale).to_string(),
        t!("menu.exit", locale = locale).to_string(),
    ]
}

pub(super) fn draw_main_menu(
    frame: &mut Frame<'_>,
    selected: usize,
    options: &[String],
    locale: &str,
) {
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

pub(super) fn draw_language_menu(frame: &mut Frame<'_>, area: Rect, selected: usize, locale: &str) {
    let options = [
        t!("language.pt", locale = locale).to_string(),
        t!("language.en", locale = locale).to_string(),
        t!("language.es", locale = locale).to_string(),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(4), Constraint::Length(3)])
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

pub(super) fn draw_settings_menu(
    frame: &mut Frame<'_>,
    area: Rect,
    selected: usize,
    options: &[String],
    status: Option<&str>,
    locale: &str,
) {
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

pub(super) fn draw_subtitle_selection(
    frame: &mut Frame<'_>,
    subtitle_streams: &[Subtitle],
    selected: usize,
    locale: &str,
    background_selection: usize,
) {
    let area = draw_main_menu_overlay(frame, background_selection, locale, 70, 70);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(6), Constraint::Length(4)])
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

pub(super) fn message_title(kind: MessageKind, locale: &str) -> (String, Color) {
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

pub(super) fn draw_message(
    frame: &mut Frame<'_>,
    area: Rect,
    kind: MessageKind,
    body: &str,
    scroll: u16,
    locale: &str,
) {
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

pub(super) fn draw_status(frame: &mut Frame<'_>, area: Rect, body: &str, locale: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
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

pub(super) fn draw_progress(
    frame: &mut Frame<'_>,
    area: Rect,
    body: &str,
    percent: u8,
    locale: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
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

    let percent = percent.min(100);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::LightGreen))
        .ratio(f64::from(percent) / 100.0)
        .label(format!("{percent}%"));
    frame.render_widget(gauge, chunks[1]);
}
