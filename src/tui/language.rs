use super::{TuiTerminal, draw_main_menu_overlay, next_selection, previous_selection, require_ui};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use rust_i18n::t;

pub(super) fn matching_language_indices(
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

pub(super) fn selected_target_language_index(
    languages: &[crate::languages::SupportedLanguage],
    default: &str,
) -> usize {
    languages
        .iter()
        .position(|language| language.code().eq_ignore_ascii_case(default))
        .unwrap_or(0)
}

pub(super) fn initial_language_selection(
    languages: &[crate::languages::SupportedLanguage],
    locale: &str,
    has_auto_option: bool,
) -> usize {
    if has_auto_option {
        0
    } else {
        selected_target_language_index(languages, &normalize_language(locale))
    }
}

pub(super) struct LanguageSelectorLabels<'a> {
    pub(super) search: &'a str,
    pub(super) field: &'a str,
    pub(super) help: &'a str,
    pub(super) auto_option: Option<&'a str>,
}

pub(super) fn matching_language_selector_indices(
    languages: &[crate::languages::SupportedLanguage],
    query: &str,
    locale: &str,
    auto_option: Option<&str>,
) -> Vec<usize> {
    let mut matching = Vec::new();
    if let Some(auto_label) = auto_option {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty()
            || "auto".contains(&normalized_query)
            || auto_label.to_lowercase().contains(&normalized_query)
        {
            matching.push(0);
        }
        matching.extend(
            matching_language_indices(languages, query, locale)
                .into_iter()
                .map(|index| index + 1),
        );
    } else {
        matching = matching_language_indices(languages, query, locale);
    }
    matching
}

pub(super) fn language_selector_code(
    languages: &[crate::languages::SupportedLanguage],
    selected: usize,
    has_auto_option: bool,
) -> String {
    if has_auto_option && selected == 0 {
        "auto".to_string()
    } else {
        let index = selected.saturating_sub(usize::from(has_auto_option));
        languages[index].code().to_string()
    }
}

pub(super) fn draw_language_selector(
    frame: &mut Frame<'_>,
    languages: &[crate::languages::SupportedLanguage],
    query: &str,
    selected: usize,
    locale: &str,
    labels: &LanguageSelectorLabels<'_>,
    background_selection: usize,
) {
    let area = draw_main_menu_overlay(frame, background_selection, locale, 75, 75);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
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
                .title(labels.search.to_string()),
        );
    frame.render_widget(search, chunks[0]);

    let filtered = matching_language_selector_indices(languages, query, locale, labels.auto_option);
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|&index| {
            if let Some(auto_label) = labels.auto_option.filter(|_| index == 0) {
                return ListItem::new(format!("{auto_label} (auto)"));
            }
            let index = index.saturating_sub(usize::from(labels.auto_option.is_some()));
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
                .title(labels.field.to_string()),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new(labels.help.to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("menu.actions", locale = locale).to_string()),
        );
    frame.render_widget(help, chunks[2]);
}

pub(super) fn target_language_selector(
    terminal: &mut TuiTerminal,
    languages: &[crate::languages::SupportedLanguage],
    locale: &str,
    background_selection: usize,
) -> Option<String> {
    let labels = LanguageSelectorLabels {
        search: &t!("translate.target_search", locale = locale),
        field: &t!("translate.target_field", locale = locale),
        help: &t!("translate.target_select_help", locale = locale),
        auto_option: None,
    };
    language_selector(terminal, languages, locale, &labels, background_selection)
}

pub(super) fn language_selector(
    terminal: &mut TuiTerminal,
    languages: &[crate::languages::SupportedLanguage],
    locale: &str,
    labels: &LanguageSelectorLabels<'_>,
    background_selection: usize,
) -> Option<String> {
    let mut query = String::new();
    let has_auto_option = labels.auto_option.is_some();
    let mut selected = initial_language_selection(languages, locale, has_auto_option);

    loop {
        let filtered =
            matching_language_selector_indices(languages, &query, locale, labels.auto_option);
        if !filtered.is_empty() {
            selected = selected.min(filtered.len() - 1);
        } else {
            selected = 0;
        }

        require_ui(terminal.draw(|frame| {
            draw_language_selector(
                frame,
                languages,
                &query,
                selected,
                locale,
                labels,
                background_selection,
            );
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
                        .map(|&index| language_selector_code(languages, index, has_auto_option));
                }
                KeyCode::Esc => break None,
                _ => {}
            }
        }
    }
}

pub(super) fn normalize_language(language: &str) -> String {
    language.trim().to_ascii_lowercase()
}
