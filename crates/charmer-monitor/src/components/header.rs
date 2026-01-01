//! Header component with dense single-line info display.

use charmer_state::PipelineState;
use chrono::Local;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct Header;

impl Header {
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
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

        let sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));

        let mut spans = Vec::new();

        // App name with status
        spans.push(Span::styled(
            "🐍 charmer",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

        // Status indicator
        if state.pipeline_finished {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "✓",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if !state.pipeline_errors.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "✗",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }

        // Run UUID (if available)
        if let Some(ref run_uuid) = state.run_uuid {
            let uuid_short = if run_uuid.len() > 8 {
                format!("{}…", &run_uuid[..8])
            } else {
                run_uuid.clone()
            };
            spans.push(sep.clone());
            spans.push(Span::styled(
                format!("[{}]", uuid_short),
                Style::default().fg(Color::Magenta),
            ));
        }

        spans.push(sep.clone());

        // Working directory
        spans.push(Span::styled(dir_display, Style::default().fg(Color::White)));

        // ETA (only if running and available)
        if let Some(eta) = state.eta_string() {
            if !state.pipeline_finished && state.pipeline_errors.is_empty() {
                spans.push(sep.clone());
                spans.push(Span::styled("ETA: ", Style::default().fg(Color::Gray)));
                spans.push(Span::styled(
                    eta,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        }

        spans.push(sep.clone());
        spans.push(Span::styled(datetime, Style::default().fg(Color::Green)));

        // Status counts (abbreviated)
        let counts = state.job_counts();
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("{} Pend", counts.pending + counts.queued),
            Style::default().fg(Color::White),
        ));
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("{} Run", counts.running),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("{} Done", counts.completed),
            Style::default().fg(Color::Green),
        ));
        spans.push(sep);
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
