use crate::{interfaces::probe::Subtitle, localization};
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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use rust_i18n::t;
use std::io;

type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

fn require_ui<T, E>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("{}", t!("errors.tui_failure")))
}
#[derive(Debug, PartialEq, Eq)]
enum MenuAction {
    Extract,
    Noop,
    Language,
    Exit,
}

fn menu_action(selected: usize) -> MenuAction {
    match selected {
        0 => MenuAction::Extract,
        3 => MenuAction::Language,
        4 => MenuAction::Exit,
        _ => MenuAction::Noop,
    }
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

pub fn main_menu() -> bool {
    let mut terminal = start_terminal();
    let mut selected = 0usize;

    let should_extract = 'menu: loop {
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
                KeyCode::Esc => break 'menu false,
                KeyCode::Enter => match menu_action(selected) {
                    MenuAction::Extract => break 'menu true,
                    MenuAction::Language => language_menu(&mut terminal),
                    MenuAction::Exit => break 'menu false,
                    MenuAction::Noop => continue,
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
    let default_output_name = default_output_name(input);

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

    finalize_output_name(&default_output_name, &value)
}

pub fn select_subtitle(subtitle_streams: &[Subtitle]) -> usize {
    let mut terminal = start_terminal();

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
                KeyCode::Char('q') | KeyCode::Esc => {
                    restore_terminal(&mut terminal);
                    std::process::exit(0);
                }
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, subtitle_streams.len()),
                KeyCode::Enter => break,
                _ => {}
            }
        }
    }

    restore_terminal(&mut terminal);

    selected
}

#[cfg(test)]
mod tests {
    use super::{
        MenuAction, default_output_name, draw_language_menu, draw_main_menu,
        draw_subtitle_selection, finalize_output_name, main_menu_options, menu_action,
        next_selection, previous_selection,
    };
    use crate::interfaces::probe::Subtitle;
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
        assert_eq!(menu_action(1), MenuAction::Noop);
        assert_eq!(menu_action(2), MenuAction::Noop);
        assert_eq!(menu_action(3), MenuAction::Language);
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
    fn selection_movement_stays_within_list_boundaries() {
        assert_eq!(previous_selection(0), 0);
        assert_eq!(previous_selection(2), 1);
        assert_eq!(next_selection(0, 5), 1);
        assert_eq!(next_selection(4, 5), 4);
        assert_eq!(next_selection(0, 0), 0);
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
