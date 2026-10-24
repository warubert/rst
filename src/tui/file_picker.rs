use super::prompts::prompt_line_in_terminal;
use super::{
    TuiTerminal, draw_main_menu_overlay, next_selection, previous_selection, require_ui, theme,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use rust_i18n::t;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub(super) enum FilePickerFilter {
    Video,
    Subtitle,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FilePickerEntry {
    Parent,
    Directory(PathBuf),
    File(PathBuf),
}

pub(super) struct FilePickerView<'a> {
    pub(super) directory: &'a Path,
    pub(super) query: &'a str,
    pub(super) entries: &'a [FilePickerEntry],
    pub(super) selected: usize,
    pub(super) error: Option<&'a str>,
    pub(super) filter: FilePickerFilter,
    pub(super) background_selection: usize,
    pub(super) locale: &'a str,
}

pub(super) fn matches_file_picker_filter(path: &Path, filter: FilePickerFilter) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match filter {
        FilePickerFilter::Video => matches!(extension.as_str(), "mkv" | "webm" | "mp4" | "mov"),
        FilePickerFilter::Subtitle => extension == "srt",
    }
}

pub(super) fn read_file_picker_entries(
    directory: &Path,
    filter: FilePickerFilter,
) -> Result<Vec<FilePickerEntry>, String> {
    let entries = fs::read_dir(directory).map_err(|error| {
        t!(
            "errors.file_picker_read",
            path = directory.display(),
            error = error
        )
        .to_string()
    })?;
    let mut directories = Vec::new();
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            t!(
                "errors.file_picker_read",
                path = directory.display(),
                error = error
            )
            .to_string()
        })?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| {
            t!(
                "errors.file_picker_read",
                path = path.display(),
                error = error
            )
            .to_string()
        })?;

        if metadata.is_dir() {
            directories.push(FilePickerEntry::Directory(path));
        } else if metadata.is_file() && matches_file_picker_filter(&path, filter) {
            files.push(FilePickerEntry::File(path));
        }
    }

    let sort_by_name = |entries: &mut Vec<FilePickerEntry>| {
        entries.sort_by_key(|entry| match entry {
            FilePickerEntry::Directory(path) | FilePickerEntry::File(path) => path
                .file_name()
                .map(|name| name.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default(),
            FilePickerEntry::Parent => String::new(),
        });
    };
    sort_by_name(&mut directories);
    sort_by_name(&mut files);

    let mut result = Vec::new();
    if directory.parent().is_some_and(|parent| parent != directory) {
        result.push(FilePickerEntry::Parent);
    }
    result.extend(directories);
    result.extend(files);
    Ok(result)
}

pub(super) fn file_picker_entry_label(entry: &FilePickerEntry) -> String {
    match entry {
        FilePickerEntry::Parent => t!("tui.parent_directory").to_string(),
        FilePickerEntry::Directory(path) => {
            format!(
                "{}/",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        }
        FilePickerEntry::File(path) => path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    }
}

pub(super) fn file_picker_entry_style(entry: &FilePickerEntry, filter: FilePickerFilter) -> Style {
    match entry {
        FilePickerEntry::File(path) if matches_file_picker_filter(path, filter) => {
            Style::default().fg(theme::FILE)
        }
        _ => Style::default(),
    }
}

pub(super) fn filter_file_picker_entries(
    entries: &[FilePickerEntry],
    query: &str,
) -> Vec<FilePickerEntry> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| match entry {
            FilePickerEntry::Parent => query.is_empty(),
            FilePickerEntry::Directory(path) | FilePickerEntry::File(path) => {
                query.is_empty()
                    || path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_lowercase().contains(&query))
                        .unwrap_or(false)
            }
        })
        .cloned()
        .collect()
}

pub(super) fn draw_file_picker(frame: &mut Frame<'_>, view: FilePickerView<'_>) {
    let area = draw_main_menu_overlay(frame, view.background_selection, view.locale, 75, 85);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(4),
        ])
        .split(area);

    let location = Paragraph::new(view.directory.display().to_string())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t!("tui.file_picker_path", locale = view.locale).to_string())
                .border_style(theme::border())
                .title_style(theme::title()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(location, chunks[0]);

    let search = Paragraph::new(view.query.to_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(t!("tui.file_picker_search", locale = view.locale).to_string())
            .border_style(theme::border())
            .title_style(theme::title()),
    );
    frame.render_widget(search, chunks[1]);

    let title = match view.filter {
        FilePickerFilter::Video => t!("tui.input_prompt", locale = view.locale).to_string(),
        FilePickerFilter::Subtitle => {
            t!("translate.srt_path_prompt", locale = view.locale).to_string()
        }
    };
    if let Some(error) = view.error {
        let error = Paragraph::new(error.to_string())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(theme::border())
                    .title_style(theme::title()),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(error, chunks[2]);
    } else if view.entries.is_empty() {
        let empty =
            Paragraph::new(t!("tui.file_picker_no_matches", locale = view.locale).to_string())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(theme::border())
                        .title_style(theme::title()),
                )
                .wrap(Wrap { trim: true });
        frame.render_widget(empty, chunks[2]);
    } else {
        let items = view
            .entries
            .iter()
            .map(|entry| {
                ListItem::new(file_picker_entry_label(entry))
                    .style(file_picker_entry_style(entry, view.filter))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(view.selected.min(items.len() - 1)));
        }
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(theme::border())
                    .title_style(theme::title()),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
        frame.render_stateful_widget(list, chunks[2], &mut state);
    }

    let help = Paragraph::new(t!("tui.file_picker_help", locale = view.locale).to_string())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border()),
        );
    frame.render_widget(help, chunks[3]);
}

pub(super) fn file_picker_in_terminal(
    terminal: &mut TuiTerminal,
    filter: FilePickerFilter,
    background_selection: usize,
) -> Result<Option<String>, String> {
    let mut directory = std::env::current_dir()
        .map_err(|error| t!("errors.file_picker_start", error = error).to_string())?;
    let (mut entries, mut error) = match read_file_picker_entries(&directory, filter) {
        Ok(entries) => (entries, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let mut selected = 0;
    let mut query = String::new();

    loop {
        let visible_entries = filter_file_picker_entries(&entries, &query);
        if visible_entries.is_empty() {
            selected = 0;
        } else {
            selected = selected.min(visible_entries.len() - 1);
        }
        let locale = rust_i18n::locale();
        require_ui(terminal.draw(|frame| {
            draw_file_picker(
                frame,
                FilePickerView {
                    directory: &directory,
                    query: &query,
                    entries: &visible_entries,
                    selected,
                    error: error.as_deref(),
                    filter,
                    background_selection,
                    locale: locale.as_ref(),
                },
            );
        }));

        if let Event::Key(key) = require_ui(event::read()) {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => selected = previous_selection(selected),
                KeyCode::Down => selected = next_selection(selected, visible_entries.len()),
                KeyCode::Backspace => {
                    if !query.is_empty() {
                        query.pop();
                        selected = 0;
                    } else if let Some(parent) = directory.parent() {
                        directory = parent.to_path_buf();
                        selected = 0;
                        match read_file_picker_entries(&directory, filter) {
                            Ok(new_entries) => {
                                entries = new_entries;
                                error = None;
                            }
                            Err(read_error) => {
                                entries.clear();
                                error = Some(read_error);
                            }
                        }
                    }
                }
                KeyCode::Char(character) if !character.is_control() => {
                    query.push(character);
                    selected = 0;
                }
                KeyCode::F(2) => {
                    if let Some(path) = prompt_line_in_terminal(
                        terminal,
                        &t!("tui.manual_path_prompt"),
                        &t!("tui.input_path"),
                        None,
                        false,
                        background_selection,
                    ) {
                        break Ok(Some(path));
                    }
                }
                KeyCode::Enter if error.is_none() => {
                    match visible_entries.get(selected) {
                        Some(FilePickerEntry::Parent) => {
                            if let Some(parent) = directory.parent() {
                                directory = parent.to_path_buf();
                                query.clear();
                            }
                        }
                        Some(FilePickerEntry::Directory(path)) => {
                            directory = path.clone();
                            query.clear();
                        }
                        Some(FilePickerEntry::File(path)) => {
                            break Ok(Some(path.to_string_lossy().into_owned()));
                        }
                        None => continue,
                    }
                    selected = 0;
                    match read_file_picker_entries(&directory, filter) {
                        Ok(new_entries) => {
                            entries = new_entries;
                            error = None;
                        }
                        Err(read_error) => {
                            entries.clear();
                            error = Some(read_error);
                        }
                    }
                }
                KeyCode::Esc => break Ok(None),
                _ => {}
            }
        }
    }
}
