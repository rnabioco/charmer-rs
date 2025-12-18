//! Rule summary component showing aggregated statistics per rule.

use charmer_state::{JobStatus, PipelineState};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

/// Statistics for a single rule.
#[derive(Debug, Default)]
pub struct RuleStats {
    pub total: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub pending: usize,
    pub total_runtime_secs: u64,
}

impl RuleStats {
    /// Calculate average runtime in seconds.
    pub fn avg_runtime_secs(&self) -> Option<u64> {
        if self.completed > 0 {
            Some(self.total_runtime_secs / self.completed as u64)
        } else {
            None
        }
    }
}

pub struct RuleSummary;

impl RuleSummary {
    /// Render the rule summary table.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        state: &PipelineState,
        rule_names: &[String],
        selected: Option<usize>,
    ) {
        // Calculate stats for each rule
        let stats: Vec<(&String, RuleStats)> = rule_names
            .iter()
            .map(|rule| {
                let job_ids = state.jobs_by_rule.get(rule);
                let mut stats = RuleStats::default();

                if let Some(ids) = job_ids {
                    for id in ids {
                        if let Some(job) = state.jobs.get(id) {
                            stats.total += 1;
                            match job.status {
                                JobStatus::Running => stats.running += 1,
                                JobStatus::Completed => {
                                    stats.completed += 1;
                                    // Calculate runtime
                                    if let (Some(start), Some(end)) =
                                        (job.timing.started_at, job.timing.completed_at)
                                    {
                                        let runtime = (end - start).num_seconds().max(0) as u64;
                                        stats.total_runtime_secs += runtime;
                                    }
                                }
                                JobStatus::Failed => stats.failed += 1,
                                JobStatus::Pending | JobStatus::Queued => stats.pending += 1,
                                _ => {}
                            }
                        }
                    }
                }
                (rule, stats)
            })
            .collect();

        // Build table rows
        let rows: Vec<Row> = stats
            .iter()
            .enumerate()
            .map(|(i, (rule, s))| {
                let is_selected = selected == Some(i);
                let base_style = if is_selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Format average runtime
                let avg_time = match s.avg_runtime_secs() {
                    Some(secs) => format_duration(secs),
                    None => "-".to_string(),
                };

                // Progress bar for this rule
                let progress = if s.total > 0 {
                    format!("{}%", s.completed * 100 / s.total)
                } else {
                    "-".to_string()
                };

                Row::new(vec![
                    Span::styled((*rule).clone(), base_style.fg(Color::Cyan)),
                    Span::styled(s.total.to_string(), base_style.fg(Color::White)),
                    Span::styled(
                        s.running.to_string(),
                        base_style.fg(if s.running > 0 {
                            Color::Yellow
                        } else {
                            Color::Gray
                        }),
                    ),
                    Span::styled(
                        s.completed.to_string(),
                        base_style.fg(if s.completed > 0 {
                            Color::Green
                        } else {
                            Color::Gray
                        }),
                    ),
                    Span::styled(
                        s.failed.to_string(),
                        base_style.fg(if s.failed > 0 {
                            Color::Red
                        } else {
                            Color::Gray
                        }),
                    ),
                    Span::styled(avg_time, base_style.fg(Color::Yellow)),
                    Span::styled(progress, base_style.fg(Color::White)),
                ])
            })
            .collect();

        // Build header
        let header = Row::new(vec![
            Span::styled(
                "Rule",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Total",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Run",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Done",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Fail",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Avg Time",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Progress",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

        let title = Line::from(vec![
            Span::styled(" Rules ", Style::default().fg(Color::White)),
            Span::styled(
                format!("({}) ", rule_names.len()),
                Style::default().fg(Color::Gray),
            ),
            Span::styled("[r: jobs view]", Style::default().fg(Color::DarkGray)),
        ]);

        let table = Table::new(
            rows,
            [
                Constraint::Min(15),    // Rule
                Constraint::Length(6),  // Total
                Constraint::Length(5),  // Running
                Constraint::Length(5),  // Done
                Constraint::Length(5),  // Failed
                Constraint::Length(10), // Avg Time
                Constraint::Length(10), // Progress
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut table_state = TableState::default();
        table_state.select(selected);

        frame.render_stateful_widget(table, area, &mut table_state);
    }
}

/// Format seconds as human-readable duration.
fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h{}m", hours, mins)
    } else if secs >= 60 {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}m{}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}
