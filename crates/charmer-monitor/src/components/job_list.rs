//! Job list component with progress indicator.

use charmer_state::{Job, JobCounts, JobStatus, PipelineState, MAIN_PIPELINE_JOB_ID};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Minimum widths for columns
const MIN_ROW_WIDTH: u16 = 4;
const MIN_STATUS_WIDTH: u16 = 2;
const MIN_RULE_WIDTH: u16 = 12;
const MAX_RULE_WIDTH: u16 = 24; // Cap rule column to prevent excessive width
const MIN_SAMPLE_WIDTH: u16 = 10;
const MIN_SLURM_WIDTH: u16 = 10;

/// Column visibility thresholds (panel width needed to show column)
const SAMPLE_THRESHOLD: u16 = 40;
const SLURM_THRESHOLD: u16 = 60;

/// Display options for job list items
struct DisplayOptions {
    content_width: u16,
    show_sample: bool,
    show_slurm: bool,
    has_scheduler_jobs: bool,
}

pub struct JobList;

impl JobList {
    /// Render the job list using filtered job IDs.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        state: &PipelineState,
        filtered_job_ids: &[String],
        selected: Option<usize>,
        filter_label: &str,
        sort_label: &str,
    ) {
        let counts = state.job_counts();

        // Count visible jobs excluding MAIN_PIPELINE_JOB_ID
        let visible = filtered_job_ids
            .iter()
            .filter(|id| *id != MAIN_PIPELINE_JOB_ID)
            .count();

        // Split area: progress bar on top, column headers, list below
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Progress header
                Constraint::Length(1), // Column headers
                Constraint::Min(1),    // Job list
            ])
            .split(area);

        // Render progress header with filter/sort in title
        render_progress_header(
            frame,
            chunks[0],
            &counts,
            visible,
            state.total_jobs,
            filter_label,
            sort_label,
        );

        // Calculate available width for content (minus borders)
        let content_width = chunks[1].width.saturating_sub(2);

        // Check if any job has a SLURM ID to determine column type
        let has_scheduler_jobs = state.jobs.values().any(|j| j.slurm_job_id.is_some());

        // Determine which columns to show based on width
        let opts = DisplayOptions {
            content_width,
            show_sample: content_width >= SAMPLE_THRESHOLD,
            show_slurm: content_width >= SLURM_THRESHOLD,
            has_scheduler_jobs,
        };

        // Render column headers
        render_column_headers(frame, chunks[1], &opts);

        // Build job list items with responsive columns
        // Track display row separately to exclude MAIN_PIPELINE_JOB_ID from numbering
        let mut display_row = 0usize;
        let items: Vec<ListItem> = filtered_job_ids
            .iter()
            .enumerate()
            .map(|(i, job_id)| {
                let row_num = if job_id == MAIN_PIPELINE_JOB_ID {
                    0
                } else {
                    display_row += 1;
                    display_row
                };
                build_job_item(row_num, i, job_id, state, &counts, selected, &opts)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM));

        let mut list_state = ListState::default();
        list_state.select(selected);

        frame.render_stateful_widget(list, chunks[2], &mut list_state);
    }
}

/// Build a single job list item with responsive columns.
fn build_job_item(
    row_num: usize,
    index: usize,
    job_id: &str,
    state: &PipelineState,
    counts: &JobCounts,
    selected: Option<usize>,
    opts: &DisplayOptions,
) -> ListItem<'static> {
    // Handle main pipeline job specially
    if job_id == MAIN_PIPELINE_JOB_ID {
        return build_main_pipeline_item(state, counts, selected == Some(index));
    }

    // Regular job
    let Some(job) = state.jobs.get(job_id) else {
        return ListItem::new(Line::from(Span::raw("???")));
    };

    let is_selected = selected == Some(index);
    let status_style = get_status_style(job.status);

    // Extract sample from wildcards
    let sample = extract_sample(job);

    // Calculate column widths
    let fixed_width = MIN_ROW_WIDTH + MIN_STATUS_WIDTH;
    let mut remaining = opts.content_width.saturating_sub(fixed_width);

    // Reserve space for optional columns
    let sample_width = if opts.show_sample {
        let w = MIN_SAMPLE_WIDTH.min(remaining / 3);
        remaining = remaining.saturating_sub(w + 1); // +1 for separator
        w
    } else {
        0
    };

    let slurm_width = if opts.show_slurm {
        let w = MIN_SLURM_WIDTH.min(remaining / 3);
        remaining = remaining.saturating_sub(w + 1);
        w
    } else {
        0
    };

    // Rule gets remaining space, capped at MAX_RULE_WIDTH
    let rule_width = remaining.clamp(MIN_RULE_WIDTH, MAX_RULE_WIDTH) as usize;

    // Build spans
    let mut spans = Vec::new();

    // Row number (highlighted when selected)
    let row_style = if is_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    spans.push(Span::styled(format!("{:3} ", row_num), row_style));

    // Status symbol (highlighted when selected)
    // Use ◎ for target rules (like "all"), otherwise use status symbol
    let status_symbol = if job.is_target {
        "◎"
    } else {
        job.status.symbol()
    };
    let status_display_style = if is_selected {
        status_style.add_modifier(Modifier::BOLD)
    } else {
        status_style
    };
    spans.push(Span::styled(
        format!("{} ", status_symbol),
        status_display_style,
    ));

    // Rule name (takes available space, truncates if needed)
    let rule_display = truncate_str(&job.rule, rule_width);
    let rule_style = if is_selected {
        status_style.add_modifier(Modifier::BOLD)
    } else {
        status_style
    };
    spans.push(Span::styled(
        format!("{:<width$}", rule_display, width = rule_width),
        rule_style,
    ));

    // Sample column (if width allows)
    if opts.show_sample {
        let sep_style = if is_selected {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(" │ ", sep_style));
        let sample_display = truncate_str(&sample, sample_width as usize);
        let sample_style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        spans.push(Span::styled(
            format!("{:<width$}", sample_display, width = sample_width as usize),
            sample_style,
        ));
    }

    // Third column: Job ID or Runtime (if width allows)
    if opts.show_slurm {
        let sep_style = if is_selected {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(" │ ", sep_style));

        let (col_display, col_style) = if opts.has_scheduler_jobs {
            // Show SLURM/LSF job ID or "local" for localrules
            let display = job
                .slurm_job_id
                .as_deref()
                .map(|s| truncate_str(s, slurm_width as usize))
                .unwrap_or_else(|| "local".to_string());
            let style = if job.slurm_job_id.is_some() {
                if is_selected {
                    Style::default().fg(Color::Gray)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            } else {
                // "local" in a different color
                Style::default().fg(Color::Magenta)
            };
            (display, style)
        } else {
            // No scheduler - show runtime instead
            let runtime = get_job_runtime(job);
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            (truncate_str(&runtime, slurm_width as usize), style)
        };

        spans.push(Span::styled(
            format!("{:<width$}", col_display, width = slurm_width as usize),
            col_style,
        ));
    }

    ListItem::new(Line::from(spans))
}

/// Get runtime string for a job.
fn get_job_runtime(job: &Job) -> String {
    use chrono::Utc;

    if let Some(started) = job.timing.started_at {
        let elapsed = if let Some(completed) = job.timing.completed_at {
            completed - started
        } else {
            Utc::now() - started
        };

        let secs = elapsed.num_seconds().unsigned_abs();
        let mins = secs / 60;
        let secs = secs % 60;

        if mins >= 60 {
            let hours = mins / 60;
            let mins = mins % 60;
            format!("{}h{}m", hours, mins)
        } else if mins > 0 {
            format!("{}m{}s", mins, secs)
        } else {
            format!("{}s", secs)
        }
    } else {
        "-".to_string()
    }
}

/// Build the main pipeline job item.
fn build_main_pipeline_item(
    state: &PipelineState,
    counts: &JobCounts,
    is_selected: bool,
) -> ListItem<'static> {
    let status_symbol = if state.pipeline_finished {
        "✓"
    } else if !state.pipeline_errors.is_empty() {
        "✗"
    } else {
        "▶"
    };

    let status_color = if state.pipeline_finished {
        Color::Green
    } else if !state.pipeline_errors.is_empty() {
        Color::Red
    } else {
        Color::Cyan
    };

    let label = if let Some(total) = state.total_jobs {
        format!("snakemake ({}/{})", counts.completed, total)
    } else {
        "snakemake (main log)".to_string()
    };

    let mut item_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    if is_selected {
        item_style = item_style.add_modifier(Modifier::REVERSED);
    }

    ListItem::new(Line::from(vec![
        Span::styled("  - ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} ", status_symbol),
            Style::default().fg(status_color),
        ),
        Span::styled(label, item_style),
    ]))
}

/// Get the style for a job status.
fn get_status_style(status: JobStatus) -> Style {
    match status {
        JobStatus::Running => Style::default().fg(Color::Yellow),
        JobStatus::Completed => Style::default().fg(Color::Green),
        JobStatus::Failed => Style::default().fg(Color::Red),
        JobStatus::Queued => Style::default().fg(Color::Blue),
        JobStatus::Pending => Style::default().fg(Color::White),
        JobStatus::Cancelled => Style::default().fg(Color::Magenta),
        JobStatus::Unknown => Style::default().fg(Color::DarkGray),
    }
}

/// Extract sample name from job wildcards.
fn extract_sample(job: &Job) -> String {
    let Some(wildcards) = &job.wildcards else {
        return String::new();
    };

    // Parse wildcards like "sample=sample1, chrom=chr1"
    // Prioritize "sample" key, fall back to first value
    for part in wildcards.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if key.trim() == "sample" {
                return value.trim().to_string();
            }
        }
    }

    // No "sample" key found, use first wildcard value
    if let Some(first) = wildcards.split(',').next() {
        if let Some((_, value)) = first.trim().split_once('=') {
            return value.trim().to_string();
        }
    }

    String::new()
}

/// Truncate a string to fit within a given width.
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width <= 1 {
        "…".to_string()
    } else {
        format!("{}…", &s[..max_width - 1])
    }
}

/// Render column headers for the job list.
fn render_column_headers(frame: &mut Frame, area: Rect, opts: &DisplayOptions) {
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let sep_style = Style::default().fg(Color::DarkGray);

    // Calculate column widths (same logic as build_job_item)
    let fixed_width = MIN_ROW_WIDTH + MIN_STATUS_WIDTH;
    let mut remaining = opts.content_width.saturating_sub(fixed_width);

    let sample_width = if opts.show_sample {
        let w = MIN_SAMPLE_WIDTH.min(remaining / 3);
        remaining = remaining.saturating_sub(w + 1);
        w
    } else {
        0
    };

    let slurm_width = if opts.show_slurm {
        let w = MIN_SLURM_WIDTH.min(remaining / 3);
        remaining = remaining.saturating_sub(w + 1);
        w
    } else {
        0
    };

    let rule_width = remaining.clamp(MIN_RULE_WIDTH, MAX_RULE_WIDTH) as usize;

    // Build header spans
    let mut spans = Vec::new();

    // Row number column header
    spans.push(Span::styled("  # ", header_style));

    // Status column header (just a symbol placeholder)
    spans.push(Span::styled("○ ", header_style));

    // Rule column header
    spans.push(Span::styled(
        format!("{:<width$}", "Rule", width = rule_width),
        header_style,
    ));

    // Sample column header
    if opts.show_sample {
        spans.push(Span::styled(" │ ", sep_style));
        spans.push(Span::styled(
            format!("{:<width$}", "Sample", width = sample_width as usize),
            header_style,
        ));
    }

    // Third column header (Job ID or Runtime)
    if opts.show_slurm {
        spans.push(Span::styled(" │ ", sep_style));
        let header_text = if opts.has_scheduler_jobs {
            "Job ID"
        } else {
            "Runtime"
        };
        spans.push(Span::styled(
            format!("{:<width$}", header_text, width = slurm_width as usize),
            header_style,
        ));
    }

    let header_line = Line::from(spans);
    let paragraph =
        Paragraph::new(header_line).block(Block::default().borders(Borders::LEFT | Borders::RIGHT));

    frame.render_widget(paragraph, area);
}

/// Render a progress header with inline progress bar.
fn render_progress_header(
    frame: &mut Frame,
    area: Rect,
    counts: &JobCounts,
    visible: usize,
    total_jobs: Option<usize>,
    filter_label: &str,
    sort_label: &str,
) {
    // Prefer total_jobs from snakemake log (more accurate) over counted jobs
    let total = total_jobs.unwrap_or(counts.total);

    // Build the title line with counts, filter, and sort
    let title = format!(
        " Jobs ({}/{}) │ Filter: {} │ Sort: {} ",
        visible, total, filter_label, sort_label
    );

    // Create progress bar with [▮▮▮▮────](n/m) style
    // Leave room for the count suffix like "(27/28)"
    let count_suffix = format!("({}/{})", visible, total);
    let bar_width = (area.width.saturating_sub(4) as usize) // borders + padding
        .saturating_sub(count_suffix.len() + 3); // brackets + space
    let filled = if total > 0 {
        (bar_width as f64 * visible as f64 / total as f64) as usize
    } else {
        0
    };

    // Build the progress bar with Unicode characters
    let bar_filled: String = "▮".repeat(filled.min(bar_width));
    let bar_empty: String = "─".repeat(bar_width.saturating_sub(filled));

    // Status summary line with bold text
    let bold = Modifier::BOLD;
    let status_line = Line::from(vec![
        Span::styled(
            format!("{} Run", counts.running),
            Style::default().fg(Color::Yellow).add_modifier(bold),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} Done", counts.completed),
            Style::default().fg(Color::Green).add_modifier(bold),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} Fail", counts.failed),
            Style::default()
                .fg(if counts.failed > 0 {
                    Color::Red
                } else {
                    Color::DarkGray
                })
                .add_modifier(bold),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} Pend", counts.queued + counts.pending),
            Style::default().fg(Color::Blue).add_modifier(bold),
        ),
        Span::raw("  "),
        Span::styled("[", Style::default().fg(Color::White).add_modifier(bold)),
        Span::styled(bar_filled, Style::default().fg(Color::Green)),
        Span::styled(bar_empty, Style::default().fg(Color::DarkGray)),
        Span::styled("]", Style::default().fg(Color::White).add_modifier(bold)),
        Span::styled(
            count_suffix,
            Style::default().fg(Color::White).add_modifier(bold),
        ),
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
