//! DAG (Directed Acyclic Graph) visualization component using Canvas widget.

use charmer_state::{JobStatus, PipelineState};
use ratatui::{
    layout::Rect,
    style::Color,
    symbols,
    widgets::{canvas::{Canvas, Circle, Line as CanvasLine}, Block, Borders},
    Frame,
};
use std::collections::HashMap;

pub struct DagView;

impl DagView {
    /// Render the DAG visualization showing job dependencies.
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
        // Group jobs by rule for visualization
        let mut rule_groups: HashMap<String, Vec<(String, JobStatus)>> = HashMap::new();

        for (job_id, job) in &state.jobs {
            if job.is_target {
                continue; // Skip target jobs like "all"
            }
            rule_groups
                .entry(job.rule.clone())
                .or_default()
                .push((job_id.clone(), job.status));
        }

        // Sort rules for consistent display
        let mut rules: Vec<_> = rule_groups.keys().cloned().collect();
        rules.sort();

        // Calculate layout - arrange rules in a grid
        let num_rules = rules.len();
        if num_rules == 0 {
            let empty = Canvas::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" DAG View - No jobs yet "),
                )
                .x_bounds([0.0, 100.0])
                .y_bounds([0.0, 100.0])
                .marker(symbols::Marker::Braille)
                .paint(|_ctx| {
                    // Empty canvas
                });
            frame.render_widget(empty, area);
            return;
        }

        // Arrange rules in a circular layout
        let canvas = Canvas::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" DAG View - {} rules ", num_rules))
                    .title_bottom(" Press 'd' to return to jobs view "),
            )
            .x_bounds([0.0, 100.0])
            .y_bounds([0.0, 100.0])
            .marker(symbols::Marker::Braille)
            .paint(|ctx| {
                let center_x = 50.0;
                let center_y = 50.0;
                let radius = 35.0;

                // Draw rules in a circle
                for (i, rule_name) in rules.iter().enumerate() {
                    let angle = (i as f64 / num_rules as f64) * 2.0 * std::f64::consts::PI;
                    let x = center_x + radius * angle.cos();
                    let y = center_y + radius * angle.sin();

                    // Get job stats for this rule
                    let jobs = rule_groups.get(rule_name).unwrap();
                    let (completed, running, failed, _pending) = jobs.iter().fold(
                        (0, 0, 0, 0),
                        |(c, r, f, p), (_, status)| match status {
                            JobStatus::Completed => (c + 1, r, f, p),
                            JobStatus::Running => (c, r + 1, f, p),
                            JobStatus::Failed => (c, r, f + 1, p),
                            JobStatus::Pending | JobStatus::Queued => (c, r, f, p + 1),
                            _ => (c, r, f, p),
                        },
                    );

                    // Choose color based on status
                    let color = if failed > 0 {
                        Color::Red
                    } else if running > 0 {
                        Color::Yellow
                    } else if completed == jobs.len() {
                        Color::Green
                    } else {
                        Color::Blue
                    };

                    // Draw node as a circle
                    ctx.draw(&Circle {
                        x,
                        y,
                        radius: 3.0,
                        color,
                    });

                    // Draw small circles for each job within the rule
                    let job_count = jobs.len().min(8); // Limit display
                    for (j, (_, status)) in jobs.iter().take(job_count).enumerate() {
                        let job_angle = angle + ((j as f64 / job_count as f64) - 0.5) * 0.3;
                        let job_radius = radius + 5.0;
                        let job_x = center_x + job_radius * job_angle.cos();
                        let job_y = center_y + job_radius * job_angle.sin();

                        let job_color = match status {
                            JobStatus::Running => Color::Yellow,
                            JobStatus::Completed => Color::Green,
                            JobStatus::Failed => Color::Red,
                            JobStatus::Queued => Color::Cyan,
                            JobStatus::Pending => Color::Blue,
                            _ => Color::Gray,
                        };

                        ctx.draw(&Circle {
                            x: job_x,
                            y: job_y,
                            radius: 0.8,
                            color: job_color,
                        });
                    }
                }

                // Draw connections between rules based on shared inputs/outputs
                // (simplified - just connect rules in sequence as example)
                for i in 0..rules.len().saturating_sub(1) {
                    let angle1 = (i as f64 / num_rules as f64) * 2.0 * std::f64::consts::PI;
                    let angle2 = ((i + 1) as f64 / num_rules as f64) * 2.0 * std::f64::consts::PI;

                    let x1 = center_x + radius * angle1.cos();
                    let y1 = center_y + radius * angle1.sin();
                    let x2 = center_x + radius * angle2.cos();
                    let y2 = center_y + radius * angle2.sin();

                    ctx.draw(&CanvasLine {
                        x1,
                        y1,
                        x2,
                        y2,
                        color: Color::DarkGray,
                    });
                }
            });

        frame.render_widget(canvas, area);
    }
}
