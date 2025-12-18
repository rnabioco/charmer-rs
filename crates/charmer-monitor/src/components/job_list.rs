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
    ) {
        let counts = state.job_counts();

        // Split area: progress bar on top, list below
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);

        // Render progress header
        render_progress_header(
            frame,
            chunks[0],
            &counts,
            filtered_job_ids.len(),
            state.total_jobs,
        );

        // Calculate available width for content (minus borders)
        let content_width = chunks[1].width.saturating_sub(2);

        // Determine which columns to show based on width
        let opts = DisplayOptions {
            content_width,
            show_sample: content_width >= SAMPLE_THRESHOLD,
            show_slurm: content_width >= SLURM_THRESHOLD,
        };

        // Build job list items with responsive columns
        let items: Vec<ListItem> = filtered_job_ids
            .iter()
            .enumerate()
            .map(|(i, job_id)| build_job_item(i, job_id, state, &counts, selected, &opts))
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM));

        let mut list_state = ListState::default();
        list_state.select(selected);

        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }
}

/// Build a single job list item with responsive columns.
fn build_job_item(
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

    // Rule gets remaining space
    let rule_width = remaining.max(MIN_RULE_WIDTH) as usize;

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
    spans.push(Span::styled(format!("{:3} ", index), row_style));

    // Status symbol (highlighted when selected)
    let status_display_style = if is_selected {
        status_style.add_modifier(Modifier::BOLD)
    } else {
        status_style
    };
    spans.push(Span::styled(
        format!("{} ", job.status.symbol()),
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

    // SLURM ID column (if width allows)
    if opts.show_slurm {
        let sep_style = if is_selected {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(" │ ", sep_style));
        let slurm_display = job
            .slurm_job_id
            .as_deref()
            .map(|s| truncate_str(s, slurm_width as usize))
            .unwrap_or_else(|| "-".to_string());
        let slurm_style = if is_selected {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(
            format!("{:<width$}", slurm_display, width = slurm_width as usize),
            slurm_style,
        ));
    }

    ListItem::new(Line::from(spans))
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

/// Render a progress header with inline progress bar.
fn render_progress_header(
    frame: &mut Frame,
    area: Rect,
    counts: &JobCounts,
    visible: usize,
    total_jobs: Option<usize>,
) {
    // Prefer total_jobs from snakemake log (more accurate) over counted jobs
    let total = total_jobs.unwrap_or(counts.total);

    // Calculate progress percentage
    let progress = if total > 0 {
        (counts.completed as f64 / total as f64 * 100.0).min(100.0) as u16
    } else {
        0
    };

    // Build the title line with counts
    let title = format!(" Jobs ({}/{}) ", visible, total);

    // Create a background progress bar effect
    let bar_width = area.width.saturating_sub(2) as usize; // Account for borders
    let filled = (bar_width as f64 * counts.completed as f64 / total.max(1) as f64) as usize;

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
