use crate::{interfaces::probe::Subtitle, localization};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use rust_i18n::t;
use std::io;

type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

fn require_ui<T, E>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("{}", t!("errors.tui_failure")))
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

pub fn main_menu() -> bool {
    let mut terminal = start_terminal();
    let mut selected = 0usize;

    let should_extract = 'menu: loop {
        let options = [
            t!("menu.extract").to_string(),
            t!("menu.translate").to_string(),
            t!("menu.extract_translate").to_string(),
            t!("menu.options").to_string(),
            t!("menu.exit").to_string(),
        ];

        require_ui(terminal.draw(|frame| {
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
                        .title(t!("menu.title").to_string())
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

            let help = Paragraph::new(t!("menu.help").to_string())
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(t!("menu.actions").to_string()),
                );
            frame.render_widget(help, chunks[3]);
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(options.len() - 1),
                KeyCode::Esc => break 'menu false,
                KeyCode::Enter => match selected {
                    0 => break 'menu true,
                    3 => language_menu(&mut terminal),
                    4 => break 'menu false,
                    _ => continue,
                },
                _ => {}
            }
        }
    };

    restore_terminal(&mut terminal);

    should_extract
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
        let options = [
            t!("language.pt").to_string(),
            t!("language.en").to_string(),
            t!("language.es").to_string(),
        ];

        require_ui(terminal.draw(|frame| {
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
                        .title(t!("language.title").to_string())
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

            let help = Paragraph::new(t!("language.help").to_string())
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(t!("menu.actions").to_string()),
                );
            frame.render_widget(help, chunks[1]);
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(locales.len() - 1),
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
    let mut terminal = start_terminal();
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
                    restore_terminal(&mut terminal);
                    return None;
                }
                KeyCode::Enter if !value.trim().is_empty() => break,
                _ => {}
            }
        }
    }

    restore_terminal(&mut terminal);

    Some(value.trim().to_string())
}

pub fn prompt_output_name(input: &str) -> String {
    let default_output_name = input
        .rsplit_once('.')
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| input.to_string());

    let mut terminal = start_terminal();

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

    restore_terminal(&mut terminal);

    let final_name = if value.trim().is_empty() {
        default_output_name.clone()
    } else {
        value
    };

    if final_name.to_lowercase().ends_with(".srt") {
        final_name
    } else {
        format!("{}.srt", final_name)
    }
}

pub fn select_subtitle(subtitle_streams: &[Subtitle]) -> usize {
    let mut terminal = start_terminal();

    let mut selected = 0usize;

    loop {
        require_ui(terminal.draw(|frame| {
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
                        .title(t!("tui.subtitles").to_string())
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

            let help = Paragraph::new(t!("tui.subtitle_help").to_string())
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(t!("menu.actions").to_string()),
                );

            frame.render_widget(help, chunks[1]);
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    restore_terminal(&mut terminal);
                    std::process::exit(0);
                }
                KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if selected + 1 < subtitle_streams.len() {
                        selected += 1;
                    }
                }
                KeyCode::Enter => break,
                _ => {}
            }
        }
    }

    restore_terminal(&mut terminal);

    selected
}
