//! View tabs component - generates title with inline tab selection.

use crate::app::ViewMode;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub struct ViewTabs;

impl ViewTabs {
    /// Generate a title Line with inline tab selection.
    /// Returns something like: " [Jobs] Rules "
    pub fn title_line(view_mode: ViewMode) -> Line<'static> {
        let tabs = [("Jobs", ViewMode::Jobs), ("Rules", ViewMode::Rules)];

        let mut spans = Vec::new();
        spans.push(Span::raw(" "));

        for (i, (name, mode)) in tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
            }

            if *mode == view_mode {
                // Selected tab - bold and highlighted
                spans.push(Span::styled(
                    format!("[{}]", name),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                // Unselected tab - dimmed
                spans.push(Span::styled(
                    name.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        spans.push(Span::raw(" "));

        Line::from(spans)
    }
}
