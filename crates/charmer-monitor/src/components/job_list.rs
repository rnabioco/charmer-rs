//! Job list component with progress indicator.

use charmer_state::{JobCounts, JobStatus, PipelineState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

pub struct JobList;

impl JobList {
    /// Render the job list using filtered job IDs.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        state: &PipelineState,
        filtered_job_ids: &[String],
        selected: Option<usize>,
    ) {
        let counts = state.job_counts();

        // Split area: progress bar on top, list below
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);

        // Render progress header
        render_progress_header(frame, chunks[0], &counts, filtered_job_ids.len());

        // Build job list items
        let items: Vec<ListItem> = filtered_job_ids
            .iter()
            .enumerate()
            .filter_map(|(i, job_id)| {
                let job = state.jobs.get(job_id)?;

                let status_style = match job.status {
                    JobStatus::Running => Style::default().fg(Color::Yellow),
                    JobStatus::Completed => Style::default().fg(Color::Green),
                    JobStatus::Failed => Style::default().fg(Color::Red),
                    JobStatus::Queued => Style::default().fg(Color::Blue),
                    JobStatus::Pending => Style::default().fg(Color::White),
                    JobStatus::Cancelled => Style::default().fg(Color::Magenta),
                    JobStatus::Unknown => Style::default().fg(Color::DarkGray),
                };

                let wildcards = job.wildcards.as_deref().unwrap_or("");
                let label = if wildcards.is_empty() {
                    job.rule.clone()
                } else {
                    format!("{}[{}]", job.rule, wildcards)
                };

                // Truncate long labels
                let label = if label.len() > 35 {
                    format!("{}...", &label[..32])
                } else {
                    label
                };

                let mut item_style = status_style;
                if selected == Some(i) {
                    item_style = item_style.add_modifier(Modifier::REVERSED);
                }

                Some(ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", job.status.symbol()), status_style),
                    Span::styled(label, item_style),
                ])))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        let mut list_state = ListState::default();
        list_state.select(selected);

        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }
}

/// Render a progress header with inline progress bar.
fn render_progress_header(frame: &mut Frame, area: Rect, counts: &JobCounts, visible: usize) {
    // Calculate progress percentage
    let progress = if counts.total > 0 {
        (counts.completed as f64 / counts.total as f64 * 100.0) as u16
    } else {
        0
    };

    // Build the title line with counts
    let title = format!(" Jobs ({}/{}) ", visible, counts.total);

    // Create a background progress bar effect
    let bar_width = area.width.saturating_sub(2) as usize; // Account for borders
    let filled = (bar_width as f64 * counts.completed as f64 / counts.total.max(1) as f64) as usize;

    // Build the progress bar as a styled line
    let bar_filled: String = "█".repeat(filled.min(bar_width));
    let bar_empty: String = "░".repeat(bar_width.saturating_sub(filled));

    // Status summary line
    let status_line = Line::from(vec![
        Span::styled(
            format!("{}R ", counts.running),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}C ", counts.completed),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!("{}F ", counts.failed),
            Style::default().fg(if counts.failed > 0 {
                Color::Red
            } else {
                Color::DarkGray
            }),
        ),
        Span::styled(
            format!("{}Q", counts.queued + counts.pending),
            Style::default().fg(Color::Blue),
        ),
        Span::raw("  "),
        Span::styled(bar_filled, Style::default().fg(Color::Green)),
        Span::styled(bar_empty, Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {}%", progress), Style::default().fg(Color::White)),
    ]);

    let block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .title(title)
        .title_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(status_line).block(block);

    frame.render_widget(paragraph, area);
}
