//! Job list component.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
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
                let style = match job.status {
                    JobStatus::Running => Style::default().fg(Color::Yellow),
                    JobStatus::Completed => Style::default().fg(Color::Green),
                    JobStatus::Failed => Style::default().fg(Color::Red),
                    _ => Style::default(),
                };

                let wildcards = job.wildcards.as_deref().unwrap_or("");
                let label = if wildcards.is_empty() {
                    job.rule.clone()
                } else {
                    format!("{}[{}]", job.rule, wildcards)
                };

                ListItem::new(format!("{} {}", job.status.symbol(), label)).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Jobs"))
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_widget(list, area);
    }
}
