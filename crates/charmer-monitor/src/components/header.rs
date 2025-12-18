//! Header component with progress bar.

use charmer_state::PipelineState;
use chrono::Local;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge},
    Frame,
};

pub struct Header;

impl Header {
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
        let counts = state.job_counts();

        // Prefer total_jobs from snakemake log (more accurate) over counted jobs
        let total = state.total_jobs.unwrap_or(counts.total);

        let progress = if total > 0 {
            (counts.completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Current date/time for right side
        let now = Local::now();
        let datetime = now.format("%Y-%m-%d %H:%M:%S").to_string();

        // Show pipeline status if finished or has errors
        // Truncate working dir to fit in header, showing last components
        let working_dir = state.working_dir.as_str();
        let max_dir_len = (area.width as usize).saturating_sub(45); // Leave room for status + datetime
        let dir_display = if working_dir.len() > max_dir_len && max_dir_len > 3 {
            format!("…{}", &working_dir[working_dir.len() - max_dir_len + 1..])
        } else {
            working_dir.to_string()
        };

        let title = if state.pipeline_finished {
            Line::from(vec![
                Span::raw("🐍 charmer "),
                Span::styled("✓ Complete", Style::default().fg(Color::Green)),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(dir_display, Style::default().fg(Color::Cyan)),
            ])
        } else if !state.pipeline_errors.is_empty() {
            Line::from(vec![
                Span::raw("🐍 charmer "),
                Span::styled("✗ Error", Style::default().fg(Color::Red)),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(dir_display, Style::default().fg(Color::Cyan)),
            ])
        } else {
            Line::from(vec![
                Span::raw("🐍 charmer"),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(dir_display, Style::default().fg(Color::Cyan)),
            ])
        };

        let datetime_line = Line::from(Span::styled(datetime, Style::default().fg(Color::Yellow)))
            .alignment(Alignment::Right);

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_top(datetime_line),
            )
            .percent(progress.min(100.0) as u16)
            .label(format!("{}/{} jobs", counts.completed, total));

        frame.render_widget(gauge, area);
    }
}
