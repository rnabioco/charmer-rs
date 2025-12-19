//! Job list component with progress indicator and dependency visualization.

use crate::app::ViewMode;
use crate::components::ViewTabs;
use charmer_state::{Job, JobCounts, JobStatus, PipelineState, MAIN_PIPELINE_JOB_ID};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};
use std::collections::{HashMap, HashSet};

/// Minimum widths for columns
const MIN_ROW_WIDTH: u16 = 4;
const MIN_STATUS_WIDTH: u16 = 2;
const MIN_RULE_WIDTH: u16 = 12;
const MAX_RULE_WIDTH: u16 = 20; // Cap rule column to prevent excessive width
const MIN_WILDCARDS_WIDTH: u16 = 16;
const MAX_WILDCARDS_WIDTH: u16 = 30; // Give wildcards more room
const RUNTIME_WIDTH: u16 = 6; // Fixed width for runtime (e.g., "1h23m" or "45m12s")
const CHAIN_WIDTH: u16 = 3; // Fixed width for dependency chain indicator

/// Column visibility thresholds (panel width needed to show column)
const WILDCARDS_THRESHOLD: u16 = 45;
const RUNTIME_THRESHOLD: u16 = 65;

/// Display options for job list items
struct DisplayOptions {
    content_width: u16,
    show_wildcards: bool,
    show_runtime: bool,
}

/// Dependency relationship to the selected job
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepRelation {
    /// This is the selected job
    Selected,
    /// This job is an upstream dependency (selected depends on this)
    Upstream,
    /// This job is a downstream dependent (this depends on selected)
    Downstream,
    /// No relation to selected job
    None,
}

/// Position in the dependency chain for rendering tree connectors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChainPosition {
    /// First node in chain (top) - uses ┐
    First,
    /// Last node in chain (bottom) - uses ┘
    Last,
    /// Middle node - uses ┤
    Middle,
    /// Not a node, just trunk passing through - uses │
    Trunk,
    /// Outside the chain entirely
    Outside,
}

/// Compute dependency relationships for all jobs relative to selected job.
/// Returns (relation, chain_position) for each job.
/// This finds the FULL transitive dependency chain (all ancestors and descendants).
fn compute_dependencies(
    state: &PipelineState,
    job_ids: &[String],
    selected_idx: Option<usize>,
) -> Vec<(DepRelation, ChainPosition)> {
    let mut relations = vec![(DepRelation::None, ChainPosition::Outside); job_ids.len()];

    let Some(sel_idx) = selected_idx else {
        return relations;
    };

    let Some(selected_id) = job_ids.get(sel_idx) else {
        return relations;
    };

    // Skip if main pipeline job is selected
    if selected_id == MAIN_PIPELINE_JOB_ID {
        return relations;
    }

    if !state.jobs.contains_key(selected_id) {
        return relations;
    }

    // Mark selected job
    relations[sel_idx].0 = DepRelation::Selected;

    // Build output->job_id map for finding upstream dependencies
    let mut output_to_job: HashMap<&str, &str> = HashMap::new();
    for (job_id, job) in &state.jobs {
        for output in &job.outputs {
            output_to_job.insert(output.as_str(), job_id.as_str());
        }
    }

    // Build input->job_ids map for finding downstream dependencies
    let mut input_to_jobs: HashMap<&str, Vec<&str>> = HashMap::new();
    for (job_id, job) in &state.jobs {
        for input in &job.inputs {
            input_to_jobs
                .entry(input.as_str())
                .or_default()
                .push(job_id.as_str());
        }
    }

    // Find all transitive upstream dependencies (ancestors)
    let mut upstream_ids: HashSet<&str> = HashSet::new();
    let mut to_visit: Vec<&str> = vec![selected_id.as_str()];
    let mut visited: HashSet<&str> = HashSet::new();

    while let Some(current_id) = to_visit.pop() {
        if visited.contains(current_id) {
            continue;
        }
        visited.insert(current_id);

        if let Some(job) = state.jobs.get(current_id) {
            for input in &job.inputs {
                if let Some(&parent_id) = output_to_job.get(input.as_str()) {
                    if parent_id != selected_id.as_str() {
                        upstream_ids.insert(parent_id);
                    }
                    if !visited.contains(parent_id) {
                        to_visit.push(parent_id);
                    }
                }
            }
        }
    }

    // Find all transitive downstream dependencies (descendants)
    let mut downstream_ids: HashSet<&str> = HashSet::new();
    let mut to_visit: Vec<&str> = vec![selected_id.as_str()];
    let mut visited: HashSet<&str> = HashSet::new();

    while let Some(current_id) = to_visit.pop() {
        if visited.contains(current_id) {
            continue;
        }
        visited.insert(current_id);

        if let Some(job) = state.jobs.get(current_id) {
            // Find jobs that consume this job's outputs
            for output in &job.outputs {
                if let Some(child_ids) = input_to_jobs.get(output.as_str()) {
                    for &child_id in child_ids {
                        if child_id != selected_id.as_str() {
                            downstream_ids.insert(child_id);
                        }
                        if !visited.contains(child_id) {
                            to_visit.push(child_id);
                        }
                    }
                }
            }
        }
    }

    // Mark relationships and collect chain member indices
    let mut chain_indices: Vec<usize> = vec![sel_idx];
    for (idx, job_id) in job_ids.iter().enumerate() {
        if idx == sel_idx {
            continue;
        }
        if upstream_ids.contains(job_id.as_str()) {
            relations[idx].0 = DepRelation::Upstream;
            chain_indices.push(idx);
        } else if downstream_ids.contains(job_id.as_str()) {
            relations[idx].0 = DepRelation::Downstream;
            chain_indices.push(idx);
        }
    }

    // Determine chain positions for tree rendering
    if chain_indices.len() > 1 {
        chain_indices.sort();
        let min_idx = chain_indices[0];
        let max_idx = chain_indices[chain_indices.len() - 1];

        // Mark positions for all rows in the range
        #[allow(clippy::needless_range_loop)]
        for idx in min_idx..=max_idx {
            if relations[idx].0 != DepRelation::None {
                // This is an actual chain member
                if idx == min_idx {
                    relations[idx].1 = ChainPosition::First;
                } else if idx == max_idx {
                    relations[idx].1 = ChainPosition::Last;
                } else {
                    relations[idx].1 = ChainPosition::Middle;
                }
            } else {
                // Trunk passes through
                relations[idx].1 = ChainPosition::Trunk;
            }
        }
    } else if chain_indices.len() == 1 {
        // Only selected job, no dependencies to show
        relations[sel_idx].1 = ChainPosition::Outside;
    }

    relations
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
        is_active: bool,
    ) {
        let counts = state.job_counts();

        // Build the outer block with tabs and active indicator
        let border_style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Tabs title on left
        let tabs_title = ViewTabs::title_line_styled(ViewMode::Jobs, is_active);

        // Active indicator on right
        let active_indicator = if is_active { " [active] " } else { "" };
        let active_style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(tabs_title)
            .title_top(Line::from(Span::styled(active_indicator, active_style)).right_aligned());

        let inner_area = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        // Split inner area: gap, progress bar, column headers, list
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Gap after title
                Constraint::Length(1), // Progress content
                Constraint::Length(1), // Column headers
                Constraint::Min(1),    // Job list
            ])
            .split(inner_area);

        // Render progress content (filter/sort/gauge)
        render_progress_content(
            frame,
            chunks[1],
            &counts,
            state.total_jobs,
            filter_label,
            sort_label,
        );

        // Calculate available width for content
        let content_width = chunks[2].width;

        // Determine which columns to show based on width
        let opts = DisplayOptions {
            content_width,
            show_wildcards: content_width >= WILDCARDS_THRESHOLD,
            show_runtime: content_width >= RUNTIME_THRESHOLD,
        };

        // Render column headers
        render_column_headers(frame, chunks[2], &opts);

        // Compute dependency relationships for visual indicator
        let deps = compute_dependencies(state, filtered_job_ids, selected);

        // Build job list items with responsive columns
        // Track display row number separately (main pipeline job doesn't get a number)
        let mut display_row = 0usize;
        let items: Vec<ListItem> = filtered_job_ids
            .iter()
            .enumerate()
            .map(|(i, job_id)| {
                let row_num = if job_id == MAIN_PIPELINE_JOB_ID {
                    0 // Main pipeline uses special display, row num not shown
                } else {
                    display_row += 1;
                    display_row
                };
                let (relation, chain_pos) = deps[i];
                build_job_item(
                    row_num, i, job_id, state, &counts, selected, &opts, relation, chain_pos,
                )
            })
            .collect();

        let list = List::new(items);

        let mut list_state = ListState::default();
        list_state.select(selected);

        frame.render_stateful_widget(list, chunks[3], &mut list_state);

        // Render scrollbar if there are more items than visible
        let list_height = chunks[3].height as usize;
        if filtered_job_ids.len() > list_height {
            let mut scrollbar_state = ScrollbarState::new(filtered_job_ids.len())
                .position(selected.unwrap_or(0))
                .viewport_content_length(list_height);

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            // Position scrollbar in the list area
            let scrollbar_area = Rect {
                x: inner_area.x,
                y: chunks[3].y,
                width: inner_area.width,
                height: chunks[3].height,
            };

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

/// Build a single job list item with responsive columns.
#[allow(clippy::too_many_arguments)]
fn build_job_item(
    row_num: usize,
    list_index: usize,
    job_id: &str,
    state: &PipelineState,
    counts: &JobCounts,
    selected: Option<usize>,
    opts: &DisplayOptions,
    dep_relation: DepRelation,
    chain_pos: ChainPosition,
) -> ListItem<'static> {
    // Handle main pipeline job specially
    if job_id == MAIN_PIPELINE_JOB_ID {
        return build_main_pipeline_item(state, counts, selected == Some(list_index));
    }

    // Regular job
    let Some(job) = state.jobs.get(job_id) else {
        return ListItem::new(Line::from(Span::raw("???")));
    };

    let is_selected = selected == Some(list_index);
    let status_style = get_status_style(job.status);

    // Extract wildcards for colored display
    let wildcards = extract_wildcards(job);

    // Calculate column widths
    // Layout: # | Status | Rule | Wildcards | Runtime | Chain
    // Chain is always at far right (fixed 3 chars)
    let fixed_width = MIN_ROW_WIDTH + MIN_STATUS_WIDTH + CHAIN_WIDTH;
    let mut remaining = opts.content_width.saturating_sub(fixed_width);

    // Reserve fixed width for runtime (rightmost before chain)
    let runtime_width = if opts.show_runtime {
        remaining = remaining.saturating_sub(RUNTIME_WIDTH + 1); // +1 for separator
        RUNTIME_WIDTH
    } else {
        0
    };

    // Reserve space for wildcards column (generous width)
    let wildcards_width = if opts.show_wildcards {
        let w = remaining
            .saturating_sub(MAX_RULE_WIDTH + 1) // leave room for rule
            .clamp(MIN_WILDCARDS_WIDTH, MAX_WILDCARDS_WIDTH);
        remaining = remaining.saturating_sub(w + 1); // +1 for separator
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
    // Use 🎯 for target rules (like "all"), otherwise use status symbol
    let status_symbol = if job.is_target {
        "🎯"
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

    // Wildcards column (if width allows) - colored with pipe separators
    if opts.show_wildcards {
        let sep_style = if is_selected {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(" │ ", sep_style));

        // Build colored wildcard spans: value1|value2|value3
        let mut wildcard_spans: Vec<Span> = Vec::new();
        let mut total_len = 0usize;
        let max_len = wildcards_width as usize;

        for (i, value) in wildcards.iter().enumerate() {
            if i > 0 {
                // Add pipe separator
                if total_len < max_len {
                    wildcard_spans.push(Span::styled("|", sep_style));
                    total_len += 1;
                } else {
                    break;
                }
            }

            // Get color for this wildcard (cycle through palette)
            let base_color = WILDCARD_COLORS[i % WILDCARD_COLORS.len()];
            let style = if is_selected {
                Style::default().fg(base_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(base_color)
            };

            // Truncate value if needed
            let remaining_space = max_len.saturating_sub(total_len);
            if remaining_space == 0 {
                break;
            }
            let display_value = if value.len() <= remaining_space {
                value.clone()
            } else if remaining_space > 1 {
                format!("{}…", &value[..remaining_space - 1])
            } else {
                "…".to_string()
            };
            total_len += display_value.len();
            wildcard_spans.push(Span::styled(display_value, style));
        }

        // Pad to column width
        let padding = max_len.saturating_sub(total_len);
        if padding > 0 {
            wildcard_spans.push(Span::raw(" ".repeat(padding)));
        }

        spans.extend(wildcard_spans);
    }

    // Runtime column (fixed width, right-aligned)
    if opts.show_runtime {
        let sep_style = if is_selected {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(" │ ", sep_style));

        let runtime = get_job_runtime(job);
        let runtime_style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };
        spans.push(Span::styled(
            format!("{:>width$}", runtime, width = runtime_width as usize),
            runtime_style,
        ));
    }

    // Dependency tree indicator (always at far right, fixed width)
    // Format: ○─┐ (first), ○─┤ (middle), ○─┘ (last), or just │ (trunk)
    // Dot style: ○ pending, ● completed, ◐ running
    let tree_style = Style::default().fg(Color::White);

    // Choose dot based on job status
    let status_dot = match job.status {
        JobStatus::Running => "◐",
        JobStatus::Completed => "●",
        _ => "○", // Pending, Queued, Failed, etc.
    };

    let dep_indicator: Vec<Span> = match chain_pos {
        ChainPosition::First => {
            let dot_style = match dep_relation {
                DepRelation::Upstream => Style::default().fg(Color::Cyan),
                DepRelation::Selected => Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
                DepRelation::Downstream => Style::default().fg(Color::Magenta),
                DepRelation::None => tree_style,
            };
            vec![
                Span::styled(status_dot, dot_style),
                Span::styled("─┐", tree_style),
            ]
        }
        ChainPosition::Middle => {
            let dot_style = match dep_relation {
                DepRelation::Upstream => Style::default().fg(Color::Cyan),
                DepRelation::Selected => Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
                DepRelation::Downstream => Style::default().fg(Color::Magenta),
                DepRelation::None => tree_style,
            };
            vec![
                Span::styled(status_dot, dot_style),
                Span::styled("─┤", tree_style),
            ]
        }
        ChainPosition::Last => {
            let dot_style = match dep_relation {
                DepRelation::Upstream => Style::default().fg(Color::Cyan),
                DepRelation::Selected => Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
                DepRelation::Downstream => Style::default().fg(Color::Magenta),
                DepRelation::None => tree_style,
            };
            vec![
                Span::styled(status_dot, dot_style),
                Span::styled("─┘", tree_style),
            ]
        }
        ChainPosition::Trunk => {
            vec![Span::styled("  │", tree_style)]
        }
        ChainPosition::Outside => {
            vec![Span::raw("   ")]
        }
    };
    spans.extend(dep_indicator);

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

/// Extract wildcards as separate values for colored display.
fn extract_wildcards(job: &Job) -> Vec<String> {
    let Some(wildcards) = &job.wildcards else {
        return Vec::new();
    };

    // Parse wildcards like "sample=sample1, chrom=chr1"
    // Return each value separately for colored rendering
    wildcards
        .split(',')
        .filter_map(|part| {
            part.trim()
                .split_once('=')
                .map(|(_, value)| value.trim().to_string())
        })
        .collect()
}

/// Color palette for wildcard values.
const WILDCARD_COLORS: [Color; 6] = [
    Color::Cyan,
    Color::Magenta,
    Color::Yellow,
    Color::Green,
    Color::Blue,
    Color::Red,
];

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
    let fixed_width = MIN_ROW_WIDTH + MIN_STATUS_WIDTH + CHAIN_WIDTH;
    let mut remaining = opts.content_width.saturating_sub(fixed_width);

    let runtime_width = if opts.show_runtime {
        remaining = remaining.saturating_sub(RUNTIME_WIDTH + 1);
        RUNTIME_WIDTH
    } else {
        0
    };

    let wildcards_width = if opts.show_wildcards {
        let w = remaining
            .saturating_sub(MAX_RULE_WIDTH + 1)
            .clamp(MIN_WILDCARDS_WIDTH, MAX_WILDCARDS_WIDTH);
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

    // Wildcards column header with rainbow "Wildcards"
    if opts.show_wildcards {
        spans.push(Span::styled(" │ ", sep_style));

        // "Samples/" in white, then "Wildcards" in rainbow
        spans.push(Span::styled("Samples/", header_style));

        // Rainbow "Wildcards" - each letter gets a color from palette
        let wildcards_text = "Wildcards";
        for (i, ch) in wildcards_text.chars().enumerate() {
            let color = WILDCARD_COLORS[i % WILDCARD_COLORS.len()];
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }

        // Pad to fill remaining width
        let header_len = "Samples/Wildcards".len();
        let padding = (wildcards_width as usize).saturating_sub(header_len);
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding)));
        }
    }

    // Runtime column header
    if opts.show_runtime {
        spans.push(Span::styled(" │ ", sep_style));
        spans.push(Span::styled(
            format!("{:>width$}", "Time", width = runtime_width as usize),
            header_style,
        ));
    }

    // Chain column - just space (no header text)
    spans.push(Span::raw("   "));

    let header_line = Line::from(spans);
    let paragraph = Paragraph::new(header_line);

    frame.render_widget(paragraph, area);
}

/// Render the progress bar section (filter/sort/gauge) without borders.
fn render_progress_content(
    frame: &mut Frame,
    area: Rect,
    counts: &JobCounts,
    total_jobs: Option<usize>,
    filter_label: &str,
    sort_label: &str,
) {
    // Prefer total_jobs from snakemake log (more accurate) over counted jobs
    let total = total_jobs.unwrap_or(counts.total);

    // Layout: Filter/Sort | Gauge | (count)
    let filter_sort_width = 8 + filter_label.len() + 7 + sort_label.len() + 2;
    let count_text = format!("({}/{})", counts.completed, total);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(filter_sort_width as u16),
            Constraint::Min(1),
            Constraint::Length(count_text.len() as u16 + 1),
        ])
        .split(area);

    // Filter/Sort label on left
    let filter_sort = Paragraph::new(Line::from(vec![
        Span::styled(" Filter:", Style::default().fg(Color::DarkGray)),
        Span::styled(filter_label, Style::default().fg(Color::Cyan)),
        Span::styled(" Sort:", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} ", sort_label),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    frame.render_widget(filter_sort, chunks[0]);

    // Gauge in middle
    let ratio = if total > 0 {
        counts.completed as f64 / total as f64
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .ratio(ratio.min(1.0));
    frame.render_widget(gauge, chunks[1]);

    // Count on right
    let count = Paragraph::new(Line::from(Span::styled(
        format!("{} ", count_text),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(count, chunks[2]);
}
