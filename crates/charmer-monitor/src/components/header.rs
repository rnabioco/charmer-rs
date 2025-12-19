//! Header component with compact status display.

use charmer_state::PipelineState;
use chrono::Local;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct Header;

impl Header {
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
        let counts = state.job_counts();

        // Current date/time
        let now = Local::now();
        let datetime = now.format("%Y-%m-%d %H:%M:%S").to_string();

        // Truncate working dir to fit in header
        let working_dir = state.working_dir.as_str();
        let max_dir_len = (area.width as usize).saturating_sub(80); // Leave room for other elements
        let dir_display = if working_dir.len() > max_dir_len && max_dir_len > 3 {
            format!("…{}", &working_dir[working_dir.len() - max_dir_len + 1..])
        } else {
            working_dir.to_string()
        };

        // Build the single-line content
        let mut spans = Vec::new();

        // App name with status
        spans.push(Span::styled(
            "🐍 charmer ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

        // Status indicator
        if state.pipeline_finished {
            spans.push(Span::styled(
                "✓",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if !state.pipeline_errors.is_empty() {
            spans.push(Span::styled(
                "✗",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                "▶",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Separator and working dir
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(dir_display, Style::default().fg(Color::White)));

        // ETA (if running and available)
        if !state.pipeline_finished && state.pipeline_errors.is_empty() {
            if let Some(eta) = state.eta_string() {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled("ETA: ", Style::default().fg(Color::Gray)));
                spans.push(Span::styled(eta, Style::default().fg(Color::Magenta)));
            }
        }

        // Separator and datetime
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(datetime, Style::default().fg(Color::Green)));

        // Separator and counts
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));

        // Abbreviated counts: "0 Pend │ 0 Run │ 4 Done │ 0 Fail"
        spans.push(Span::styled(
            format!("{} Pend", counts.pending + counts.queued),
            Style::default().fg(Color::White),
        ));
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!("{} Run", counts.running),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!("{} Done", counts.completed),
            Style::default().fg(Color::Green),
        ));
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!("{} Fail", counts.failed),
            Style::default().fg(if counts.failed > 0 {
                Color::Red
            } else {
                Color::DarkGray
            }),
        ));

        let content = Line::from(spans);

        let block = Block::default().borders(Borders::ALL);

        let paragraph = Paragraph::new(content).block(block);

        frame.render_widget(paragraph, area);
    }
}
