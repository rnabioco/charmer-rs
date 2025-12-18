//! Footer component with keyboard shortcuts.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Footer;

impl Footer {
    pub fn render(frame: &mut Frame, area: Rect) {
        let help = "j/k:navigate  l:logs  r:rules  f:filter  s:sort  ?:help  q:quit";
        let version = format!("v{}", VERSION);

        // Split footer into left (help) and right (version)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(version.len() as u16 + 1),
            ])
            .split(area);

        let help_paragraph = Paragraph::new(help).style(Style::default().fg(Color::Gray));
        frame.render_widget(help_paragraph, chunks[0]);

        let version_paragraph = Paragraph::new(Line::from(Span::styled(
            version,
            Style::default().fg(Color::Gray),
        )));
        frame.render_widget(version_paragraph, chunks[1]);
    }
}
