//! Job detail panel with rich formatting.

use charmer_state::{EnvType, ExecutionEnvironment, FailureMode, Job, JobStatus, PipelineState};
use chrono::Utc;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Color palette for wildcard values (matches job_list.rs).
const WILDCARD_COLORS: [Color; 6] = [
    Color::Cyan,
    Color::Magenta,
    Color::Yellow,
    Color::Green,
    Color::Blue,
    Color::Red,
];

pub struct JobDetail;

impl JobDetail {
    pub fn render(frame: &mut Frame, area: Rect, job: Option<&Job>, command_expanded: bool) {
        let content = match job {
            Some(job) => build_detail_lines(job, command_expanded),
            None => vec![Line::from(Span::styled(
                "No job selected",
                Style::default().fg(Color::DarkGray),
            ))],
        };

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Job Details "),
        );

        frame.render_widget(paragraph, area);
    }

    /// Render pipeline summary when main snakemake job is selected.
    pub fn render_pipeline(frame: &mut Frame, area: Rect, state: &PipelineState) {
        let content = build_pipeline_lines(state);

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Job Details "),
        );

        frame.render_widget(paragraph, area);
    }
}

/// Build detail lines for pipeline summary.
fn build_pipeline_lines(state: &PipelineState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let counts = state.job_counts();

    // Title
    lines.push(Line::from(vec![Span::styled(
        "Snakemake Pipeline",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));

    lines.push(Line::from(""));

    // Status
    let (status_text, status_color) = if state.pipeline_finished {
        ("Completed", Color::Green)
    } else if !state.pipeline_errors.is_empty() {
        ("Failed", Color::Red)
    } else {
        ("Running", Color::Yellow)
    };

    lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::Gray)),
        Span::styled(
            status_text,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Host
    if let Some(ref host) = state.host {
        lines.push(Line::from(vec![
            Span::styled("Host: ", Style::default().fg(Color::Gray)),
            Span::styled(host.clone(), Style::default().fg(Color::White)),
        ]));
    }

    // Cores
    if let Some(cores) = state.cores {
        lines.push(Line::from(vec![
            Span::styled("Cores: ", Style::default().fg(Color::Gray)),
            Span::styled(cores.to_string(), Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(""));

    // Progress section
    lines.push(Line::from(Span::styled(
        "Progress",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    // Total jobs
    if let Some(total) = state.total_jobs {
        lines.push(Line::from(vec![
            Span::styled("  Total: ", Style::default().fg(Color::Gray)),
            Span::styled(total.to_string(), Style::default().fg(Color::White)),
            Span::styled(" jobs", Style::default().fg(Color::Gray)),
        ]));

        // Progress percentage
        let progress = counts.completed as f64 / total as f64 * 100.0;
        lines.push(Line::from(vec![
            Span::styled("  Progress: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.0}%", progress),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!(" ({}/{})", counts.completed, total),
                Style::default().fg(Color::Gray),
            ),
        ]));
    }

    // Job breakdown
    lines.push(Line::from(vec![
        Span::styled("  Running: ", Style::default().fg(Color::Gray)),
        Span::styled(
            counts.running.to_string(),
            Style::default().fg(Color::Yellow),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  Completed: ", Style::default().fg(Color::Gray)),
        Span::styled(
            counts.completed.to_string(),
            Style::default().fg(Color::Green),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  Failed: ", Style::default().fg(Color::Gray)),
        Span::styled(
            counts.failed.to_string(),
            Style::default().fg(if counts.failed > 0 {
                Color::Red
            } else {
                Color::Gray
            }),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  Pending: ", Style::default().fg(Color::Gray)),
        Span::styled(
            (counts.pending + counts.queued).to_string(),
            Style::default().fg(Color::Blue),
        ),
    ]));

    // Errors section
    if !state.pipeline_errors.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Errors",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));

        for error in state.pipeline_errors.iter().take(3) {
            // Error type with icon
            let label = format!("{} {}", error.icon(), error.label());
            let mut spans = vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    label,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ];

            // Add rule name if available
            if let Some(ref rule) = error.rule {
                spans.push(Span::styled(
                    format!(" ({})", rule),
                    Style::default().fg(Color::Yellow),
                ));
            }

            // Add exit code if available
            if let Some(code) = error.exit_code {
                spans.push(Span::styled(
                    format!(" [exit {}]", code),
                    Style::default().fg(Color::Gray),
                ));
            }

            lines.push(Line::from(spans));

            // Show first detail if available
            if let Some(detail) = error.details.first() {
                let msg = if detail.len() > 42 {
                    format!("...{}", &detail[detail.len() - 39..])
                } else {
                    detail.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled(msg, Style::default().fg(Color::Gray)),
                ]));
            }
        }

        if state.pipeline_errors.len() > 3 {
            lines.push(Line::from(Span::styled(
                format!("  (+{} more)", state.pipeline_errors.len() - 3),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    lines
}

fn build_detail_lines(job: &Job, command_expanded: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Rule name with color
    lines.push(Line::from(vec![
        Span::styled("Rule: ", Style::default().fg(Color::Gray)),
        Span::styled(
            job.rule.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Wildcards / Sample info - colored to match job list
    if let Some(ref wildcards) = job.wildcards {
        let mut spans = vec![Span::styled(
            "Wildcards: ",
            Style::default().fg(Color::Gray),
        )];

        // Parse and color each wildcard: key in white, value in color
        let pairs: Vec<(&str, &str)> = wildcards
            .split(',')
            .filter_map(|part| {
                part.trim()
                    .split_once('=')
                    .map(|(k, v)| (k.trim(), v.trim()))
            })
            .collect();

        for (i, (key, value)) in pairs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(", ", Style::default().fg(Color::DarkGray)));
            }
            // Key in white
            spans.push(Span::styled(
                format!("{}=", key),
                Style::default().fg(Color::White),
            ));
            // Value in color
            let color = WILDCARD_COLORS[i % WILDCARD_COLORS.len()];
            spans.push(Span::styled(value.to_string(), Style::default().fg(color)));
        }

        lines.push(Line::from(spans));
    } else {
        // Try to extract sample from output path
        if let Some(sample) =
            extract_sample_from_path(&job.outputs.first().cloned().unwrap_or_default())
        {
            lines.push(Line::from(vec![
                Span::styled("Sample: ", Style::default().fg(Color::Gray)),
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
        Span::styled("Status: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} {}", job.status.symbol(), status_text),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Scheduler Job ID (SLURM/LSF)
    if let Some(ref slurm_id) = job.scheduler_job_id {
        lines.push(Line::from(vec![
            Span::styled("Job ID: ", Style::default().fg(Color::Gray)),
            Span::styled(slurm_id.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // Execution environment
    let env = ExecutionEnvironment::detect(
        &job.shellcmd,
        job.conda_env.as_deref(),
        job.container_img_url.as_deref(),
    );
    if env.env_type != EnvType::Direct {
        let (env_label, env_color) = match env.env_type {
            EnvType::Pixi => ("Pixi", Color::Magenta),
            EnvType::Conda => ("Conda", Color::Green),
            EnvType::Container => ("Container", Color::Blue),
            EnvType::Direct => ("Direct", Color::Gray),
        };
        let env_name = env.env_name.or(env.image_url).unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled("Env: ", Style::default().fg(Color::Gray)),
            Span::styled(
                env_label,
                Style::default().fg(env_color).add_modifier(Modifier::BOLD),
            ),
            if !env_name.is_empty() {
                Span::styled(format!(" ({})", env_name), Style::default().fg(Color::Gray))
            } else {
                Span::raw("")
            },
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
            Span::styled("  Queue: ", Style::default().fg(Color::Gray)),
            Span::styled(partition.clone(), Style::default().fg(Color::Magenta)),
        ]));
    }

    // Node
    if let Some(ref node) = job.resources.node {
        lines.push(Line::from(vec![
            Span::styled("  Node: ", Style::default().fg(Color::Gray)),
            Span::styled(node.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // CPUs
    if let Some(cpus) = job.resources.cpus {
        lines.push(Line::from(vec![
            Span::styled("  CPUs: ", Style::default().fg(Color::Gray)),
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
            Span::styled("  Memory: ", Style::default().fg(Color::Gray)),
            Span::styled(mem_str, Style::default().fg(Color::Green)),
        ]));
    }

    // Time limit
    if let Some(ref time_limit) = job.resources.time_limit {
        lines.push(Line::from(vec![
            Span::styled("  Time Limit: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_duration(time_limit),
                Style::default().fg(Color::Yellow),
            ),
        ]));
    }

    // Usage section (actual consumption for finished jobs)
    if let Some(ref usage) = job.usage {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Actual Usage",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));

        // Max RSS (actual memory used)
        if let Some(max_rss) = usage.max_rss_mb {
            let mem_str = if max_rss >= 1024 {
                format!("{:.1} GB", max_rss as f64 / 1024.0)
            } else {
                format!("{} MB", max_rss)
            };
            // Compare with requested
            let efficiency = job.resources.memory_mb.map(|req| {
                if req > 0 {
                    (max_rss as f64 / req as f64 * 100.0) as u32
                } else {
                    0
                }
            });
            let eff_color = match efficiency {
                Some(e) if e > 90 => Color::Red,    // Near limit
                Some(e) if e > 70 => Color::Yellow, // Good utilization
                Some(e) if e > 30 => Color::Green,  // Moderate
                _ => Color::Cyan,                   // Low utilization
            };
            let mut spans = vec![
                Span::styled("  Memory: ", Style::default().fg(Color::Gray)),
                Span::styled(mem_str, Style::default().fg(eff_color)),
            ];
            if let Some(eff) = efficiency {
                spans.push(Span::styled(
                    format!(" ({}%)", eff),
                    Style::default().fg(Color::Gray),
                ));
            }
            lines.push(Line::from(spans));
        }

        // Elapsed time
        if let Some(elapsed) = usage.elapsed_seconds {
            let time_str = format_seconds(elapsed);
            // Compare with time limit
            let efficiency = job.resources.time_limit.map(|limit| {
                let limit_secs = limit.as_secs();
                if limit_secs > 0 {
                    (elapsed as f64 / limit_secs as f64 * 100.0) as u32
                } else {
                    0
                }
            });
            let mut spans = vec![
                Span::styled("  Runtime: ", Style::default().fg(Color::Gray)),
                Span::styled(time_str, Style::default().fg(Color::Green)),
            ];
            if let Some(eff) = efficiency {
                spans.push(Span::styled(
                    format!(" ({}%)", eff),
                    Style::default().fg(Color::Gray),
                ));
            }
            lines.push(Line::from(spans));
        }

        // CPU time
        if let Some(cpu_time) = usage.cpu_time_seconds {
            let time_str = format_seconds(cpu_time);
            lines.push(Line::from(vec![
                Span::styled("  CPU Time: ", Style::default().fg(Color::Gray)),
                Span::styled(time_str, Style::default().fg(Color::Cyan)),
            ]));
        }
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
            Span::styled("  Wait: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_chrono_duration(&wait),
                Style::default().fg(Color::Blue),
            ),
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
            Span::styled("  Runtime: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_chrono_duration_hms(&runtime),
                Style::default().fg(runtime_color),
            ),
        ]));
    }

    // Started at
    if let Some(started) = job.timing.started_at {
        lines.push(Line::from(vec![
            Span::styled("  Started: ", Style::default().fg(Color::Gray)),
            Span::styled(
                started.format("%Y-%m-%d %H:%M:%S").to_string(),
                Style::default().fg(Color::White),
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

        // Show failure analysis if available
        if let Some(ref analysis) = error.analysis {
            // Failure mode with icon and color
            let (mode_icon, mode_text, mode_color) = match analysis.mode {
                FailureMode::OutOfMemory => ("⚠", "Out of Memory", Color::Red),
                FailureMode::Timeout => ("⏱", "Timeout", Color::Yellow),
                FailureMode::ExitCode => ("✗", "Exit Code Error", Color::Red),
                FailureMode::Cancelled => ("⊘", "Cancelled", Color::Magenta),
                FailureMode::NodeFailure => ("⚡", "Node Failure", Color::LightRed),
                FailureMode::Unknown => ("?", "Unknown", Color::Gray),
            };
            lines.push(Line::from(vec![
                Span::styled("  Failure: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} {}", mode_icon, mode_text),
                    Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
                ),
            ]));

            // Memory details for OOM
            if analysis.mode == FailureMode::OutOfMemory
                && let (Some(used), Some(limit)) =
                    (analysis.memory_used_mb, analysis.memory_limit_mb)
            {
                lines.push(Line::from(vec![
                    Span::styled("  Memory: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{:.1} GB", used as f64 / 1024.0),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" / ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{:.1} GB limit", limit as f64 / 1024.0),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }

            // Time details for Timeout
            if analysis.mode == FailureMode::Timeout
                && let (Some(runtime), Some(limit)) =
                    (analysis.runtime_seconds, analysis.time_limit_seconds)
            {
                lines.push(Line::from(vec![
                    Span::styled("  Time: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format_seconds(runtime),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" / ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{} limit", format_seconds(limit)),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }

            // Explanation
            if !analysis.explanation.is_empty() {
                // Wrap long explanations
                let explanation = if analysis.explanation.len() > 45 {
                    format!("{}...", &analysis.explanation[..42])
                } else {
                    analysis.explanation.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(explanation, Style::default().fg(Color::White)),
                ]));
            }

            // Suggestion (highlighted)
            if !analysis.suggestion.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Suggestion",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )));
                // Handle multi-line suggestions
                for line in analysis.suggestion.lines().take(3) {
                    let suggestion_line = if line.len() > 45 {
                        format!("{}...", &line[..42])
                    } else {
                        line.to_string()
                    };
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(suggestion_line, Style::default().fg(Color::Green)),
                    ]));
                }
            }
        } else {
            // No analysis available - show basic error info
            lines.push(Line::from(vec![
                Span::styled("  Exit Code: ", Style::default().fg(Color::Gray)),
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
                    Span::styled("  Message: ", Style::default().fg(Color::Gray)),
                    Span::styled(msg, Style::default().fg(Color::Red)),
                ]));
            }
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
                Span::styled(display, Style::default().fg(Color::Gray)),
            ]));
        }
        if job.outputs.len() > 3 {
            lines.push(Line::from(Span::styled(
                format!("  (+{} more)", job.outputs.len() - 3),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    // Shell command preview
    let trimmed_cmd = job.shellcmd.trim();
    if !trimmed_cmd.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Command",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));

        let all_cmd_lines: Vec<&str> = trimmed_cmd
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let total_lines = all_cmd_lines.len();

        if command_expanded {
            // Show all lines, no truncation
            for cmd_line in &all_cmd_lines {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(cmd_line.to_string(), Style::default().fg(Color::Gray)),
                ]));
            }
            // Show hint to collapse/copy
            lines.push(Line::from(Span::styled(
                "  ('e' to collapse, 'c' to copy)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            // Show first 3 lines with truncation
            for cmd_line in all_cmd_lines.iter().take(3) {
                let display = if cmd_line.len() > 50 {
                    format!("{}…", &cmd_line[..49])
                } else {
                    cmd_line.to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(display, Style::default().fg(Color::Gray)),
                ]));
            }

            // Indicate if there are more lines
            if total_lines > 3 {
                lines.push(Line::from(Span::styled(
                    format!("  (+{} more lines, press 'e' to expand)", total_lines - 3),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
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

fn format_chrono_duration_hms(d: &chrono::Duration) -> String {
    let secs = d.num_seconds().unsigned_abs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

fn format_seconds(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}
