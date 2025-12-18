//! Job detail panel.

use charmer_state::Job;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct JobDetail;

impl JobDetail {
    pub fn render(frame: &mut Frame, area: Rect, job: Option<&Job>) {
        let content = match job {
            Some(job) => {
                let mut lines = vec![
                    format!("Rule: {}", job.rule),
                    format!("Status: {:?}", job.status),
                ];

                if let Some(ref slurm_id) = job.slurm_job_id {
                    lines.push(format!("SLURM Job: {}", slurm_id));
                }

                if let Some(ref node) = job.resources.node {
                    lines.push(format!("Node: {}", node));
                }

                if let Some(cpus) = job.resources.cpus {
                    lines.push(format!("CPUs: {}", cpus));
                }

                if let Some(mem) = job.resources.memory_mb {
                    lines.push(format!("Memory: {} MB", mem));
                }

                lines.join("\n")
            }
            None => "No job selected".to_string(),
        };

        let paragraph =
            Paragraph::new(content).block(Block::default().borders(Borders::ALL).title("Details"));

        frame.render_widget(paragraph, area);
    }
}
