//! Main TUI application.

use crate::components::{Footer, Header, JobDetail, JobList, LogViewer, LogViewerState};
use crate::ui::Theme;
use charmer_state::{JobStatus, PipelineState};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Clear,
    Frame,
};
use std::time::{Duration, Instant};

/// Filter mode for job list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    Running,
    Failed,
    Pending,
    Completed,
}

impl FilterMode {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Running,
            Self::Running => Self::Failed,
            Self::Failed => Self::Pending,
            Self::Pending => Self::Completed,
            Self::Completed => Self::All,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Running => "Running",
            Self::Failed => "Failed",
            Self::Pending => "Pending",
            Self::Completed => "Completed",
        }
    }

    pub fn matches(&self, status: JobStatus) -> bool {
        match self {
            Self::All => true,
            Self::Running => matches!(status, JobStatus::Running),
            Self::Failed => matches!(status, JobStatus::Failed),
            Self::Pending => matches!(status, JobStatus::Pending | JobStatus::Queued),
            Self::Completed => matches!(status, JobStatus::Completed),
        }
    }
}

/// Sort mode for job list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Status,
    Rule,
    Time,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            Self::Status => Self::Rule,
            Self::Rule => Self::Time,
            Self::Time => Self::Status,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Status => "Status",
            Self::Rule => "Rule",
            Self::Time => "Time",
        }
    }
}

/// Main application state.
pub struct App {
    pub state: PipelineState,
    pub should_quit: bool,
    pub selected_index: usize,
    pub filter_mode: FilterMode,
    pub sort_mode: SortMode,
    pub show_help: bool,
    pub show_log_viewer: bool,
    pub log_viewer_state: Option<LogViewerState>,
    pub theme: Theme,
    pub last_tick: Instant,
    job_ids: Vec<String>, // Cached sorted/filtered job IDs
}

impl App {
    pub fn new(state: PipelineState) -> Self {
        let job_ids = state.jobs.keys().cloned().collect();
        Self {
            state,
            should_quit: false,
            selected_index: 0,
            filter_mode: FilterMode::default(),
            sort_mode: SortMode::default(),
            show_help: false,
            show_log_viewer: false,
            log_viewer_state: None,
            theme: Theme::dark(),
            last_tick: Instant::now(),
            job_ids,
        }
    }

    /// Update cached job list based on filter and sort.
    pub fn update_job_list(&mut self) {
        let mut jobs: Vec<_> = self
            .state
            .jobs
            .iter()
            .filter(|(_, job)| self.filter_mode.matches(job.status))
            .collect();

        // Sort jobs
        match self.sort_mode {
            SortMode::Status => {
                jobs.sort_by_key(|(_, job)| match job.status {
                    JobStatus::Running => 0,
                    JobStatus::Failed => 1,
                    JobStatus::Queued => 2,
                    JobStatus::Pending => 3,
                    JobStatus::Completed => 4,
                    JobStatus::Cancelled => 5,
                    JobStatus::Unknown => 6,
                });
            }
            SortMode::Rule => {
                jobs.sort_by(|(_, a), (_, b)| a.rule.cmp(&b.rule));
            }
            SortMode::Time => {
                jobs.sort_by(|(_, a), (_, b)| {
                    let a_time = a.timing.started_at.or(a.timing.queued_at);
                    let b_time = b.timing.started_at.or(b.timing.queued_at);
                    b_time.cmp(&a_time) // Most recent first
                });
            }
        }

        self.job_ids = jobs.into_iter().map(|(id, _)| id.clone()).collect();

        // Clamp selection
        if !self.job_ids.is_empty() {
            self.selected_index = self.selected_index.min(self.job_ids.len() - 1);
        } else {
            self.selected_index = 0;
        }
    }

    /// Get the currently selected job.
    pub fn selected_job(&self) -> Option<&charmer_state::Job> {
        self.job_ids
            .get(self.selected_index)
            .and_then(|id| self.state.jobs.get(id))
    }

    /// Get filtered job IDs.
    pub fn filtered_jobs(&self) -> &[String] {
        &self.job_ids
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn select_next(&mut self) {
        if !self.job_ids.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.job_ids.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.job_ids.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.job_ids.len() - 1);
        }
    }

    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    pub fn select_last(&mut self) {
        if !self.job_ids.is_empty() {
            self.selected_index = self.job_ids.len() - 1;
        }
    }

    pub fn cycle_filter(&mut self) {
        self.filter_mode = self.filter_mode.next();
        self.update_job_list();
    }

    pub fn cycle_sort(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.update_job_list();
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Toggle log viewer for the currently selected job.
    pub fn toggle_log_viewer(&mut self) {
        if self.show_log_viewer {
            self.close_log_viewer();
            return;
        }
        self.open_log_viewer();
    }

    /// Find the best log file path for a job.
    fn find_log_path(&self, job: &charmer_state::Job) -> String {
        let working_dir = &self.state.working_dir;

        // Try log files from snakemake metadata first
        for log_file in &job.log_files {
            let full_path = working_dir.join(log_file);
            if full_path.exists() {
                return full_path.to_string();
            }
            // Also try as-is (might be absolute)
            if std::path::Path::new(log_file).exists() {
                return log_file.clone();
            }
        }

        // Try SLURM log path format: .snakemake/slurm_logs/rule_{rule}/{slurm_job_id}.log
        if let Some(ref slurm_id) = job.slurm_job_id {
            let slurm_log = working_dir
                .join(".snakemake")
                .join("slurm_logs")
                .join(format!("rule_{}", job.rule))
                .join(format!("{}.log", slurm_id));
            if slurm_log.exists() {
                return slurm_log.to_string();
            }
        }

        // Try common log directory patterns
        let wildcards_suffix = job
            .wildcards
            .as_ref()
            .map(|w| {
                // Extract just the values, e.g., "sample=sample1, chrom=chr1" -> "sample1"
                w.split(',')
                    .next()
                    .and_then(|s| s.split('=').nth(1))
                    .unwrap_or("")
            })
            .unwrap_or("");

        // Try logs/{rule}/{wildcard}.log
        if !wildcards_suffix.is_empty() {
            let pattern_log = working_dir
                .join("logs")
                .join(&job.rule)
                .join(format!("{}.log", wildcards_suffix));
            if pattern_log.exists() {
                return pattern_log.to_string();
            }
        }

        // Try logs/{rule}.log
        let rule_log = working_dir.join("logs").join(format!("{}.log", job.rule));
        if rule_log.exists() {
            return rule_log.to_string();
        }

        // Try main snakemake log as fallback (most recent .snakemake.log file)
        if let Some(main_log) = self.find_latest_snakemake_log() {
            return main_log;
        }

        // Fallback: return a path that shows what we're looking for
        if !job.log_files.is_empty() {
            working_dir.join(&job.log_files[0]).to_string()
        } else {
            working_dir
                .join("logs")
                .join(&job.rule)
                .join(format!("{}.log", wildcards_suffix))
                .to_string()
        }
    }

    /// Find the most recent main snakemake log file.
    fn find_latest_snakemake_log(&self) -> Option<String> {
        let log_dir = self.state.working_dir.join(".snakemake").join("log");
        if !log_dir.exists() {
            return None;
        }

        let mut latest: Option<(std::time::SystemTime, String)> = None;

        if let Ok(entries) = std::fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".snakemake.log") {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                let path_str = path.to_string_lossy().to_string();
                                if latest.is_none() || modified > latest.as_ref().unwrap().0 {
                                    latest = Some((modified, path_str));
                                }
                            }
                        }
                    }
                }
            }
        }

        latest.map(|(_, path)| path)
    }

    /// Open log viewer for the currently selected job.
    fn open_log_viewer(&mut self) {
        if let Some(job) = self.selected_job().cloned() {
            let log_path = self.find_log_path(&job);

            let mut state = LogViewerState::new(log_path, 1000);
            state.follow_mode = true; // Enable follow mode by default for panel view
            self.log_viewer_state = Some(state);
            self.show_log_viewer = true;
        }
    }

    /// Update log viewer to show the currently selected job's logs.
    fn update_log_viewer_for_selected(&mut self) {
        if let Some(job) = self.selected_job().cloned() {
            let log_path = self.find_log_path(&job);

            let mut state = LogViewerState::new(log_path, 1000);
            state.follow_mode = true;
            self.log_viewer_state = Some(state);
        }
    }

    /// Close the log viewer.
    pub fn close_log_viewer(&mut self) {
        self.show_log_viewer = false;
        self.log_viewer_state = None;
    }

    /// Refresh the log viewer content.
    pub fn refresh_log_viewer(&mut self) {
        if let Some(ref state) = self.log_viewer_state {
            let log_path = state.log_path.clone();
            let follow = state.follow_mode;
            self.log_viewer_state = Some(LogViewerState::new(log_path, 1000));
            if follow {
                if let Some(ref mut new_state) = self.log_viewer_state {
                    new_state.follow_mode = true;
                    new_state.scroll_to_bottom();
                }
            }
        }
    }

    /// Update app state from external source (polling service).
    pub fn update_from_state(&mut self, new_state: PipelineState) {
        self.state = new_state;
        self.update_job_list();

        // Refresh log viewer if in follow mode
        if self.show_log_viewer {
            if let Some(ref state) = self.log_viewer_state {
                if state.follow_mode {
                    self.refresh_log_viewer();
                }
            }
        }
    }

    /// Handle a key event.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // If help is showing, any key closes it
        if self.show_help {
            self.show_help = false;
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                // Update log viewer to show new job's logs
                if self.show_log_viewer {
                    self.update_log_viewer_for_selected();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous();
                // Update log viewer to show new job's logs
                if self.show_log_viewer {
                    self.update_log_viewer_for_selected();
                }
            }
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('f') => self.cycle_filter(),
            KeyCode::Char('s') => self.cycle_sort(),
            KeyCode::Char('l') | KeyCode::Enter => self.toggle_log_viewer(),
            KeyCode::Char('F') if self.show_log_viewer => {
                // Toggle follow mode when log panel is open
                if let Some(ref mut state) = self.log_viewer_state {
                    state.toggle_follow();
                }
            }
            KeyCode::Char('?') => self.toggle_help(),
            _ => {}
        }
    }

    /// Poll for events and handle them.
    pub fn poll_events(&mut self, timeout: Duration) -> std::io::Result<bool> {
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Render the UI.
    pub fn render(&self, frame: &mut Frame) {
        // Adjust layout based on whether log panel is open
        let chunks = if self.show_log_viewer {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header with progress
                    Constraint::Length(1),  // Status counts
                    Constraint::Min(8),     // Main content (smaller when logs open)
                    Constraint::Length(12), // Log panel
                    Constraint::Length(1),  // Footer
                ])
                .split(frame.area())
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header with progress
                    Constraint::Length(1), // Status counts
                    Constraint::Min(10),   // Main content
                    Constraint::Length(0), // No log panel
                    Constraint::Length(1), // Footer
                ])
                .split(frame.area())
        };

        // Header
        Header::render(frame, chunks[0], &self.state);

        // Status counts
        self.render_status_bar(frame, chunks[1]);

        // Main content: split horizontally
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        // Job list (left) and detail (right)
        JobList::render(
            frame,
            main_chunks[0],
            &self.state,
            &self.job_ids,
            Some(self.selected_index),
        );
        JobDetail::render(frame, main_chunks[1], self.selected_job());

        // Log panel at bottom (if open)
        if self.show_log_viewer {
            self.render_log_panel(frame, chunks[3]);
        }

        // Footer
        Footer::render(frame, chunks[4]);

        // Help overlay (on top of everything)
        if self.show_help {
            self.render_help_overlay(frame);
        }
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        let counts = self.state.job_counts();

        let spans = vec![
            Span::styled(
                format!(" {} Pending ", counts.pending + counts.queued),
                Style::default().fg(Color::White),
            ),
            Span::raw("│"),
            Span::styled(
                format!(" {} Running ", counts.running),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("│"),
            Span::styled(
                format!(" {} Done ", counts.completed),
                Style::default().fg(Color::Green),
            ),
            Span::raw("│"),
            Span::styled(
                format!(" {} Failed ", counts.failed),
                Style::default().fg(Color::Red),
            ),
            Span::raw(" │ Filter: "),
            Span::styled(self.filter_mode.label(), Style::default().fg(Color::Cyan)),
            Span::raw(" │ Sort: "),
            Span::styled(self.sort_mode.label(), Style::default().fg(Color::Cyan)),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    fn render_log_panel(&self, frame: &mut Frame, area: Rect) {
        if let Some(ref state) = self.log_viewer_state {
            // Render log viewer as a bottom panel (tailed output)
            LogViewer::render_panel(frame, area, state);
        }
    }

    fn render_help_overlay(&self, frame: &mut Frame) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let area = centered_rect(60, 60, frame.area());

        let help_text = r#"
  Keyboard Shortcuts
  ──────────────────

  j / ↓      Move down (also updates log panel)
  k / ↑      Move up (also updates log panel)
  g / Home   Go to first job
  G / End    Go to last job
  f          Cycle filter (All/Running/Failed/Pending/Completed)
  s          Cycle sort (Status/Rule/Time)
  l / Enter  Toggle log panel
  F          Toggle follow mode (when logs open)
  ?          Toggle this help
  q / Ctrl+C Quit

  Press any key to close
"#;

        frame.render_widget(Clear, area);
        let paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Help ")
                    .style(Style::default().bg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));

        frame.render_widget(paragraph, area);
    }
}

/// Create a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
