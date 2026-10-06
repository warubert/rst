use crate::interfaces::probe::Subtitle;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;

pub fn main_menu() -> bool {
    enable_raw_mode().expect("Falha ao ativar modo raw");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)
        .expect("Falha ao entrar na tela alternada");

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Falha ao inicializar terminal Ratatui");
    let options = [
        "1) Extrair legenda",
        "2) Traduzir legenda",
        "3) Extrair e traduzir",
        "4) Opções",
        "5) Sair",
    ];
    let mut selected = 0usize;

    let should_extract = loop {
        terminal
            .draw(|frame| {
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
                    .map(|option| ListItem::new(*option))
                    .collect();
                let mut list_state = ListState::default();
                list_state.select(Some(selected));

                let menu = List::new(items)
                    .block(Block::default().title("Menu principal").borders(Borders::ALL))
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::LightBlue)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                frame.render_stateful_widget(menu, chunks[2], &mut list_state);

                let help = Paragraph::new("Use as setas ↑/↓ para mover e Enter para selecionar")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title("Ações"));
                frame.render_widget(help, chunks[3]);
            })
            .expect("Falha ao renderizar interface");

        if let Event::Key(key) = event::read().expect("Falha ao ler evento do teclado") {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(options.len() - 1),
                KeyCode::Esc => break false,
                KeyCode::Enter => match selected {
                    0 => break true,
                    4 => break false,
                    _ => {}
                },
                _ => {}
            }
        }
    };

    disable_raw_mode().expect("Falha ao desabilitar modo raw");
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show,)
        .expect("Falha ao restaurar tela");

    should_extract
}

pub fn prompt_input_path() -> Option<String> {
    enable_raw_mode().expect("Falha ao ativar modo raw");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)
        .expect("Falha ao entrar na tela alternada");

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Falha ao inicializar terminal Ratatui");
    let mut value = String::new();

    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(6)
                    .constraints([Constraint::Length(4), Constraint::Length(3)])
                    .split(area);

                let title = Paragraph::new("Digite o caminho do arquivo de vídeo")
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
                            .title("Caminho do arquivo")
                            .title_alignment(Alignment::Center),
                    );
                frame.render_widget(field, chunks[1]);
            })
            .expect("Falha ao renderizar interface");

        if let Event::Key(key) = event::read().expect("Falha ao ler evento do teclado") {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char(c) if !c.is_control() => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Esc => {
                    disable_raw_mode().expect("Falha ao desabilitar modo raw");
                    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show,)
                        .expect("Falha ao restaurar tela");
                    return None;
                }
                KeyCode::Enter if !value.trim().is_empty() => break,
                _ => {}
            }
        }
    }

    disable_raw_mode().expect("Falha ao desabilitar modo raw");
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show,)
        .expect("Falha ao restaurar tela");

    Some(value.trim().to_string())
}

pub fn prompt_output_name(input: &str) -> String {
    let default_output_name = input
        .rsplit_once('.')
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| input.to_string());

    enable_raw_mode().expect("Falha ao ativar modo raw");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)
        .expect("Falha ao entrar na tela alternada");

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Falha ao inicializar terminal Ratatui");

    let mut value = String::new();

    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(6)
                    .constraints([Constraint::Length(4), Constraint::Length(3)])
                    .split(area);

                let title = Paragraph::new("Nome da legenda de saída")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title("RST"));
                frame.render_widget(title, chunks[0]);

                let display = if value.is_empty() {
                    format!("{} (padrão)", default_output_name)
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
                            .title("Arquivo .srt")
                            .title_alignment(Alignment::Center),
                    );
                frame.render_widget(field, chunks[1]);
            })
            .expect("Falha ao renderizar interface");

        if let Event::Key(key) = event::read().expect("Falha ao ler evento do teclado") {
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

    disable_raw_mode().expect("Falha ao desabilitar modo raw");
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show,)
        .expect("Falha ao restaurar tela");

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
    enable_raw_mode().expect("Falha ao ativar modo raw");

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)
        .expect("Falha ao entrar na tela alternada");

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Falha ao inicializar terminal Ratatui");

    let mut selected = 0usize;

    loop {
        terminal
            .draw(|frame| {
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
                            .title("Legendas disponíveis")
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

                let help = Paragraph::new(
                    "Use as setas ↑/↓ para mover\nEnter para confirmar\nq ou Esc para sair",
                )
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title("Ações"));

                frame.render_widget(help, chunks[1]);
            })
            .expect("Falha ao renderizar interface");

        if let Event::Key(key) = event::read().expect("Falha ao ler evento do teclado") {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    disable_raw_mode().expect("Falha ao desabilitar modo raw");
                    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show,)
                        .expect("Falha ao restaurar tela");
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

    disable_raw_mode().expect("Falha ao desabilitar modo raw");
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show,)
        .expect("Falha ao restaurar tela");

    selected
}
