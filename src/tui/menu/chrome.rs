use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders};

pub const ACCENT: Color = Color::Cyan;
pub const GOOD: Color = Color::Green;
pub const BAD: Color = Color::Red;
pub const DIM: Color = Color::DarkGray;

pub fn screen(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
}

pub fn bold(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::default().add_modifier(Modifier::BOLD))
}

pub fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(DIM))
}

pub fn good(text: impl Into<String>) -> Span<'static> {
    Span::styled(
        text.into(),
        Style::default().fg(GOOD).add_modifier(Modifier::BOLD),
    )
}

pub fn bad(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(BAD))
}

pub fn keybar(text: &str) -> ratatui::text::Line<'static> {
    ratatui::text::Line::from(dim(text.to_string()))
}
