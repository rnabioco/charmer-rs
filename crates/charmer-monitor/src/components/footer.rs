//! Footer component with keyboard shortcuts and status messages.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Footer;

impl Footer {
    pub fn render(frame: &mut Frame, area: Rect, status_message: Option<&str>) {
        let help = "j/k:nav  R:runs  a:all  l:logs  r:rules  f:filter  s:sort  ?:help  q:quit";
        let version = format!("v{}", VERSION);

        // Split footer into left (help/status), right (version)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(version.len() as u16 + 1),
            ])
            .split(area);

        // Show status message if present, otherwise show help
        let left_content = if let Some(msg) = status_message {
            Line::from(Span::styled(
                msg.to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ))
        } else {
            Line::from(Span::styled(help, Style::default().fg(Color::Gray)))
        };

        let help_paragraph = Paragraph::new(left_content);
        frame.render_widget(help_paragraph, chunks[0]);

        let version_paragraph = Paragraph::new(Line::from(Span::styled(
            version,
            Style::default().fg(Color::Gray),
        )));
        frame.render_widget(version_paragraph, chunks[1]);
    }
}
