//! Job list component.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use charmer_state::{PipelineState, JobStatus};

pub struct JobList;

impl JobList {
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState, selected: Option<usize>) {
        let items: Vec<ListItem> = state
            .jobs
            .values()
            .enumerate()
            .map(|(i, job)| {
                let mut style = match job.status {
                    JobStatus::Running => Style::default().fg(Color::Yellow),
                    JobStatus::Completed => Style::default().fg(Color::Green),
                    JobStatus::Failed => Style::default().fg(Color::Red),
                    JobStatus::Queued => Style::default().fg(Color::Blue),
                    JobStatus::Pending => Style::default().fg(Color::White),
                    JobStatus::Cancelled => Style::default().fg(Color::DarkGray),
                    JobStatus::Unknown => Style::default().fg(Color::Gray),
                };

                // Highlight selected item
                if selected == Some(i) {
                    style = style.add_modifier(Modifier::REVERSED);
                }

                let wildcards = job.wildcards.as_deref().unwrap_or("");
                let label = if wildcards.is_empty() {
                    job.rule.clone()
                } else {
                    format!("{}[{}]", job.rule, wildcards)
                };

                ListItem::new(format!("{} {}", job.status.symbol(), label)).style(style)
            })
            .collect();

        let job_count = items.len();
        let title = format!("Jobs ({}/{})", selected.map(|s| s + 1).unwrap_or(0), job_count);

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

        // Use stateful rendering for scroll support
        let mut list_state = ListState::default();
        list_state.select(selected);

        frame.render_stateful_widget(list, area, &mut list_state);
    }
}
