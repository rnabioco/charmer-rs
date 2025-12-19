//! View tabs component for switching between Jobs and Rules views.

use crate::app::ViewMode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Tabs},
    Frame,
};

pub struct ViewTabs;

impl ViewTabs {
    /// Render the view tabs.
    pub fn render(frame: &mut Frame, area: Rect, view_mode: ViewMode) {
        let titles = vec!["Jobs", "Rules", "DAG"];

        let selected = match view_mode {
            ViewMode::Jobs => 0,
            ViewMode::Rules => 1,
            ViewMode::Dag => 2,
        };

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .select(selected)
            .style(Style::default().fg(Color::Gray))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" │ ");

        frame.render_widget(tabs, area);
    }
}
