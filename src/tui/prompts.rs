use super::screens::mask_input;
use super::{TuiTerminal, draw_main_menu_overlay, require_ui, theme};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use rust_i18n::t;

pub(super) fn default_output_name(input: &str) -> String {
    let path = std::path::Path::new(input);
    if path.extension().is_some() {
        path.with_extension(String::new())
            .to_string_lossy()
            .into_owned()
    } else {
        input.to_string()
    }
}

pub(super) fn finalize_output_name(default_name: &str, value: &str) -> String {
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

pub(super) fn prompt_api_key(
    terminal: &mut TuiTerminal,
    background_selection: usize,
) -> Option<String> {
    prompt_line_in_terminal(
        terminal,
        &t!("settings.api_key_prompt"),
        &t!("settings.api_key_field"),
        None,
        true,
        background_selection,
    )
}

pub(super) fn draw_prompt_in_area(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    field_title: &str,
    display: &str,
    margin: u16,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(margin)
        .constraints([Constraint::Length(4), Constraint::Length(3)])
        .split(area);

    let title = Paragraph::new(title.to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("RST")
                .border_style(theme::border())
                .title_style(theme::title()),
        );
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
                .title_alignment(Alignment::Center)
                .border_style(theme::border())
                .title_style(theme::title()),
        );
    frame.render_widget(field, chunks[1]);
}

pub(super) fn draw_modal_prompt(
    frame: &mut Frame<'_>,
    title: &str,
    field_title: &str,
    display: &str,
    background_selection: usize,
    locale: &str,
) {
    let area = draw_main_menu_overlay(frame, background_selection, locale, 70, 45);
    draw_prompt_in_area(frame, area, title, field_title, display, 1);
}

pub(super) fn resolve_prompt_value(value: &str, default: Option<&str>) -> Option<String> {
    let value = value.trim();
    if !value.is_empty() {
        Some(value.to_string())
    } else {
        default.map(str::to_string)
    }
}

pub(super) fn prompt_line_in_terminal(
    terminal: &mut TuiTerminal,
    title: &str,
    field_title: &str,
    default: Option<&str>,
    masked: bool,
    background_selection: usize,
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
        require_ui(terminal.draw(|frame| {
            let locale = rust_i18n::locale();
            draw_modal_prompt(
                frame,
                title,
                field_title,
                &display,
                background_selection,
                locale.as_ref(),
            );
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

pub(super) fn default_translation_name(input: &str, target: &str) -> String {
    format!("{}-{}", default_output_name(input), target)
}

pub(super) fn prompt_output_name_in_terminal(
    terminal: &mut TuiTerminal,
    input: &str,
    background_selection: usize,
) -> String {
    let default_output_name = default_output_name(input);
    let value = prompt_line_in_terminal(
        terminal,
        &t!("tui.output_prompt"),
        &t!("tui.output_file"),
        Some(&default_output_name),
        false,
        background_selection,
    )
    .unwrap_or_default();
    finalize_output_name(&default_output_name, &value)
}
