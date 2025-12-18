//! Job detail panel with rich formatting.

use charmer_state::{Job, JobStatus};
use chrono::Utc;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct JobDetail;

impl JobDetail {
    pub fn render(frame: &mut Frame, area: Rect, job: Option<&Job>) {
        let content = match job {
            Some(job) => build_detail_lines(job),
            None => vec![Line::from(Span::styled(
                "No job selected",
                Style::default().fg(Color::DarkGray),
            ))],
        };

        let paragraph = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title(" Details "));

        frame.render_widget(paragraph, area);
    }
}

fn build_detail_lines(job: &Job) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Rule name with color
    lines.push(Line::from(vec![
        Span::styled("Rule: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            job.rule.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Wildcards / Sample info
    if let Some(ref wildcards) = job.wildcards {
        lines.push(Line::from(vec![
            Span::styled("Wildcards: ", Style::default().fg(Color::DarkGray)),
            Span::styled(wildcards.clone(), Style::default().fg(Color::Yellow)),
        ]));
    } else {
        // Try to extract sample from output path
        if let Some(sample) = extract_sample_from_path(&job.outputs.first().cloned().unwrap_or_default()) {
            lines.push(Line::from(vec![
                Span::styled("Sample: ", Style::default().fg(Color::DarkGray)),
                Span::styled(sample, Style::default().fg(Color::Yellow)),
            ]));
        }
    }

    lines.push(Line::from(""));

    // Status with appropriate color
    let (status_text, status_color) = match job.status {
        JobStatus::Running => ("Running", Color::Yellow),
        JobStatus::Completed => ("Completed", Color::Green),
        JobStatus::Failed => ("Failed", Color::Red),
        JobStatus::Queued => ("Queued", Color::Blue),
        JobStatus::Pending => ("Pending", Color::White),
        JobStatus::Cancelled => ("Cancelled", Color::Magenta),
        JobStatus::Unknown => ("Unknown", Color::DarkGray),
    };
    lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} {}", job.status.symbol(), status_text),
            Style::default().fg(status_color).add_modifier(Modifier::BOLD),
        ),
    ]));

    // SLURM/LSF Job ID
    if let Some(ref slurm_id) = job.slurm_job_id {
        lines.push(Line::from(vec![
            Span::styled("Job ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(slurm_id.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    lines.push(Line::from(""));

    // Resources section
    lines.push(Line::from(Span::styled(
        "Resources",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    // Partition/Queue
    if let Some(ref partition) = job.resources.partition {
        lines.push(Line::from(vec![
            Span::styled("  Queue: ", Style::default().fg(Color::DarkGray)),
            Span::styled(partition.clone(), Style::default().fg(Color::Magenta)),
        ]));
    }

    // Node
    if let Some(ref node) = job.resources.node {
        lines.push(Line::from(vec![
            Span::styled("  Node: ", Style::default().fg(Color::DarkGray)),
            Span::styled(node.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // CPUs
    if let Some(cpus) = job.resources.cpus {
        lines.push(Line::from(vec![
            Span::styled("  CPUs: ", Style::default().fg(Color::DarkGray)),
            Span::styled(cpus.to_string(), Style::default().fg(Color::Green)),
        ]));
    }

    // Memory
    if let Some(mem) = job.resources.memory_mb {
        let mem_str = if mem >= 1024 {
            format!("{:.1} GB", mem as f64 / 1024.0)
        } else {
            format!("{} MB", mem)
        };
        lines.push(Line::from(vec![
            Span::styled("  Memory: ", Style::default().fg(Color::DarkGray)),
            Span::styled(mem_str, Style::default().fg(Color::Green)),
        ]));
    }

    // Time limit
    if let Some(ref time_limit) = job.resources.time_limit {
        lines.push(Line::from(vec![
            Span::styled("  Time Limit: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_duration(time_limit), Style::default().fg(Color::Yellow)),
        ]));
    }

    lines.push(Line::from(""));

    // Timing section
    lines.push(Line::from(Span::styled(
        "Timing",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    // Wait time (queued to started)
    if let (Some(queued), Some(started)) = (job.timing.queued_at, job.timing.started_at) {
        let wait = started - queued;
        lines.push(Line::from(vec![
            Span::styled("  Wait: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_chrono_duration(&wait), Style::default().fg(Color::Blue)),
        ]));
    }

    // Runtime
    if let Some(started) = job.timing.started_at {
        let runtime = if let Some(completed) = job.timing.completed_at {
            completed - started
        } else {
            Utc::now() - started
        };
        let runtime_color = if job.status == JobStatus::Running {
            Color::Yellow
        } else {
            Color::Green
        };
        lines.push(Line::from(vec![
            Span::styled("  Runtime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_chrono_duration(&runtime), Style::default().fg(runtime_color)),
        ]));
    }

    // Started at
    if let Some(started) = job.timing.started_at {
        lines.push(Line::from(vec![
            Span::styled("  Started: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                started.format("%H:%M:%S").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // Error section (if failed)
    if let Some(ref error) = job.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Error",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(vec![
            Span::styled("  Exit Code: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                error.exit_code.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
        if !error.message.is_empty() {
            // Truncate long error messages
            let msg = if error.message.len() > 50 {
                format!("{}...", &error.message[..47])
            } else {
                error.message.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("  Message: ", Style::default().fg(Color::DarkGray)),
                Span::styled(msg, Style::default().fg(Color::Red)),
            ]));
        }
    }

    // Output files
    if !job.outputs.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Output",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        for output in job.outputs.iter().take(3) {
            let display = if output.len() > 40 {
                format!("...{}", &output[output.len() - 37..])
            } else {
                output.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(display, Style::default().fg(Color::DarkGray)),
            ]));
        }
        if job.outputs.len() > 3 {
            lines.push(Line::from(Span::styled(
                format!("  (+{} more)", job.outputs.len() - 3),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Shell command preview
    if !job.shellcmd.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Command",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        // Show first line or truncated command
        let cmd_preview: String = job
            .shellcmd
            .lines()
            .next()
            .unwrap_or(&job.shellcmd)
            .chars()
            .take(45)
            .collect();
        let cmd_display = if cmd_preview.len() < job.shellcmd.lines().next().map(|l| l.len()).unwrap_or(0) {
            format!("{}...", cmd_preview)
        } else {
            cmd_preview
        };
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(cmd_display, Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines
}

/// Extract sample name from output path patterns like "results/processed/sample1.txt"
fn extract_sample_from_path(path: &str) -> Option<String> {
    // Common patterns: look for sample names between slashes
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 {
        // Get the filename without extension
        if let Some(filename) = parts.last() {
            let name = filename.split('.').next().unwrap_or(filename);
            // Check if it looks like a sample name (not a generic name)
            if !name.is_empty() && name != "output" && name != "result" {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn format_duration(d: &std::time::Duration) -> String {
    let secs = d.as_secs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

fn format_chrono_duration(d: &chrono::Duration) -> String {
    let secs = d.num_seconds().unsigned_abs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}
