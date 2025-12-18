//! Header component with progress bar.

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Gauge},
    Frame,
};
use charmer_state::PipelineState;

pub struct Header;

impl Header {
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
        let counts = state.job_counts();
        let progress = if counts.total > 0 {
            (counts.completed as f64 / counts.total as f64) * 100.0
        } else {
            0.0
        };

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("charmer"))
            .percent(progress as u16)
            .label(format!("{}/{} jobs", counts.completed, counts.total));

        frame.render_widget(gauge, area);
    }
}
