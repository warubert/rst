use ratatui::style::{Color, Modifier, Style};

pub(super) const ACCENT: Color = Color::LightCyan;
pub(super) const FILE: Color = Color::LightGreen;
pub(super) const MUTED: Color = Color::Gray;
pub(super) const SUCCESS: Color = Color::Green;
pub(super) const INFO: Color = Color::Yellow;
pub(super) const ERROR: Color = Color::Red;

pub(super) fn border() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(super) fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub(super) fn selection() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
