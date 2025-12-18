//! Footer component with keyboard shortcuts.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

pub struct Footer;

impl Footer {
    pub fn render(frame: &mut Frame, area: Rect) {
        let help = "j/k:navigate  l:logs  f:filter  s:sort  ?:help  q:quit";
        let paragraph = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }
}
