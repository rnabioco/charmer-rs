//! Main TUI application.

use crate::components::{
    Footer, Header, JobDetail, JobList, LogViewer, LogViewerState, RuleSummary,
};
use crate::ui::Theme;
use charmer_state::{JobStatus, PipelineState, MAIN_PIPELINE_JOB_ID};
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

/// View mode for main panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Show individual jobs
    #[default]
    Jobs,
    /// Show rule summary
    Rules,
}

/// Active panel for keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    /// Job list or rule list (left panel)
    #[default]
    List,
    /// Log viewer panel (bottom)
    Logs,
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
    pub view_mode: ViewMode,
    pub active_panel: ActivePanel,
    pub show_help: bool,
    pub show_log_viewer: bool,
    pub log_viewer_state: Option<LogViewerState>,
    pub theme: Theme,
    pub last_tick: Instant,
    job_ids: Vec<String>,                      // Cached sorted/filtered job IDs
    rule_names: Vec<String>,                   // Cached rule names for rule view
    status_message: Option<(String, Instant)>, // Temporary status message with timestamp
    command_expanded: bool,                    // Whether command section is expanded in details
    waiting_for_panel_key: bool, // True when Ctrl-W was pressed, waiting for direction
}

impl App {
    pub fn new(state: PipelineState) -> Self {
        let job_ids = state.jobs.keys().cloned().collect();
        let rule_names: Vec<String> = state.jobs_by_rule.keys().cloned().collect();
        let mut app = Self {
            state,
            should_quit: false,
            selected_index: 0,
            filter_mode: FilterMode::default(),
            sort_mode: SortMode::default(),
            view_mode: ViewMode::default(),
            active_panel: ActivePanel::default(),
            show_help: false,
            show_log_viewer: false,
            log_viewer_state: None,
            theme: Theme::dark(),
            last_tick: Instant::now(),
            job_ids,
            rule_names,
            status_message: None,
            command_expanded: false,
            waiting_for_panel_key: false,
        };
        // Update job list first to ensure MAIN_PIPELINE_JOB_ID is in the list
        app.update_job_list();
        // Open log viewer by default
        app.open_log_viewer();
        app
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
                jobs.sort_by_key(|(_, job)| {
                    // Target jobs (like "all") always go to the bottom, regardless of status
                    // They represent pipeline completion and should be last
                    if job.is_target {
                        return 10;
                    }
                    match job.status {
                        JobStatus::Running => 0,
                        JobStatus::Failed => 1,
                        JobStatus::Queued => 2,
                        JobStatus::Pending => 3,
                        JobStatus::Completed => 4,
                        JobStatus::Cancelled => 5,
                        JobStatus::Unknown => 6,
                    }
                });
            }
            SortMode::Rule => {
                jobs.sort_by(|(_, a), (_, b)| {
                    // Target jobs go to the bottom in rule sort too
                    match (a.is_target, b.is_target) {
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => a.rule.cmp(&b.rule),
                    }
                });
            }
            SortMode::Time => {
                jobs.sort_by(|(_, a), (_, b)| {
                    // Target jobs go to the bottom in time sort too
                    match (a.is_target, b.is_target) {
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => {
                            let a_time = a.timing.started_at.or(a.timing.queued_at);
                            let b_time = b.timing.started_at.or(b.timing.queued_at);
                            b_time.cmp(&a_time) // Most recent first
                        }
                    }
                });
            }
        }

        // Build job IDs list with main pipeline job at top
        self.job_ids = Vec::with_capacity(jobs.len() + 1);

        // Always add main pipeline job at the top (when viewing all or running)
        if matches!(self.filter_mode, FilterMode::All | FilterMode::Running) {
            self.job_ids.push(MAIN_PIPELINE_JOB_ID.to_string());
        }

        // Add sorted job IDs
        self.job_ids
            .extend(jobs.into_iter().map(|(id, _)| id.clone()));

        // Clamp selection
        if !self.job_ids.is_empty() {
            self.selected_index = self.selected_index.min(self.job_ids.len() - 1);
        } else {
            self.selected_index = 0;
        }
    }

    /// Get the currently selected job.
    /// Returns None if the main pipeline job is selected (it's synthetic).
    pub fn selected_job(&self) -> Option<&charmer_state::Job> {
        self.job_ids.get(self.selected_index).and_then(|id| {
            if id == MAIN_PIPELINE_JOB_ID {
                None // Main pipeline job is synthetic
            } else {
                self.state.jobs.get(id)
            }
        })
    }

    /// Check if the main pipeline job is currently selected.
    pub fn is_main_pipeline_selected(&self) -> bool {
        self.job_ids
            .get(self.selected_index)
            .map(|id| id == MAIN_PIPELINE_JOB_ID)
            .unwrap_or(false)
    }

    /// Get the currently selected job ID.
    pub fn selected_job_id(&self) -> Option<&str> {
        self.job_ids.get(self.selected_index).map(|s| s.as_str())
    }

    /// Get filtered job IDs.
    pub fn filtered_jobs(&self) -> &[String] {
        &self.job_ids
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Get the list length based on current view mode.
    fn list_len(&self) -> usize {
        match self.view_mode {
            ViewMode::Jobs => self.job_ids.len(),
            ViewMode::Rules => self.rule_names.len(),
        }
    }

    pub fn select_next(&mut self) {
        let len = self.list_len();
        if len > 0 {
            self.selected_index = (self.selected_index + 1) % len;
            self.command_expanded = false; // Reset expansion when navigating
        }
    }

    pub fn select_previous(&mut self) {
        let len = self.list_len();
        if len > 0 {
            self.selected_index = self.selected_index.checked_sub(1).unwrap_or(len - 1);
            self.command_expanded = false; // Reset expansion when navigating
        }
    }

    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    pub fn select_last(&mut self) {
        let len = self.list_len();
        if len > 0 {
            self.selected_index = len - 1;
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

    /// Toggle between jobs and rules view.
    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Jobs => ViewMode::Rules,
            ViewMode::Rules => ViewMode::Jobs,
        };
        // Reset selection when switching views
        self.selected_index = 0;
        // Update rule names list when switching to rules view
        if self.view_mode == ViewMode::Rules {
            self.update_rule_list();
        }
    }

    /// Update the cached rule names list.
    fn update_rule_list(&mut self) {
        let mut rules: Vec<_> = self.state.jobs_by_rule.keys().cloned().collect();
        rules.sort();
        self.rule_names = rules;
    }

    /// Get the currently selected rule name (in rules view).
    pub fn selected_rule(&self) -> Option<&str> {
        if self.view_mode == ViewMode::Rules {
            self.rule_names.get(self.selected_index).map(|s| s.as_str())
        } else {
            None
        }
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

        // Try SLURM log path format: .snakemake/slurm_logs/rule_{rule}/{scheduler_job_id}.log
        if let Some(ref slurm_id) = job.scheduler_job_id {
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
        let log_path = if self.is_main_pipeline_selected() {
            // For main pipeline job, show the main snakemake log
            self.find_latest_snakemake_log()
                .unwrap_or_else(|| "(no snakemake log found)".to_string())
        } else if let Some(job) = self.selected_job().cloned() {
            self.find_log_path(&job)
        } else {
            return;
        };

        let mut state = LogViewerState::new(log_path, 1000);
        state.follow_mode = true; // Enable follow mode by default for panel view
        self.log_viewer_state = Some(state);
        self.show_log_viewer = true;
    }

    /// Update log viewer to show the currently selected job's logs.
    fn update_log_viewer_for_selected(&mut self) {
        let log_path = if self.is_main_pipeline_selected() {
            // For main pipeline job, show the main snakemake log
            self.find_latest_snakemake_log()
                .unwrap_or_else(|| "(no snakemake log found)".to_string())
        } else if let Some(job) = self.selected_job().cloned() {
            self.find_log_path(&job)
        } else {
            return;
        };

        let mut state = LogViewerState::new(log_path, 1000);
        state.follow_mode = true;
        self.log_viewer_state = Some(state);
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

    /// Copy the selected job's shell command to clipboard.
    fn copy_command(&mut self) {
        let job = self.selected_job();
        if let Some(job) = job {
            let cmd = job.shellcmd.trim();
            if cmd.is_empty() {
                self.status_message = Some(("No command to copy".to_string(), Instant::now()));
                return;
            }

            match arboard::Clipboard::new() {
                Ok(mut clipboard) => match clipboard.set_text(cmd) {
                    Ok(()) => {
                        self.status_message =
                            Some(("Command copied to clipboard".to_string(), Instant::now()));
                    }
                    Err(_) => {
                        self.status_message =
                            Some(("Failed to copy to clipboard".to_string(), Instant::now()));
                    }
                },
                Err(_) => {
                    self.status_message =
                        Some(("Clipboard not available".to_string(), Instant::now()));
                }
            }
        } else {
            self.status_message = Some(("No job selected".to_string(), Instant::now()));
        }
    }

    /// Scroll log viewer page up.
    fn scroll_log_page_up(&mut self) {
        if let Some(ref mut state) = self.log_viewer_state {
            for _ in 0..10 {
                state.scroll_up();
            }
        }
    }

    /// Scroll log viewer page down.
    fn scroll_log_page_down(&mut self) {
        if let Some(ref mut state) = self.log_viewer_state {
            for _ in 0..10 {
                state.scroll_down();
            }
        }
    }

    /// Switch to the next panel (cycles through available panels).
    fn switch_panel_down(&mut self) {
        if self.show_log_viewer {
            self.active_panel = match self.active_panel {
                ActivePanel::List => ActivePanel::Logs,
                ActivePanel::Logs => ActivePanel::List,
            };
        }
    }

    /// Switch to the previous panel (cycles through available panels).
    fn switch_panel_up(&mut self) {
        if self.show_log_viewer {
            self.active_panel = match self.active_panel {
                ActivePanel::List => ActivePanel::Logs,
                ActivePanel::Logs => ActivePanel::List,
            };
        }
    }

    /// Handle a key event.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // If help is showing, any key closes it
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Handle Ctrl-W panel switching (vim-style)
        if self.waiting_for_panel_key {
            self.waiting_for_panel_key = false;
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.switch_panel_down(),
                KeyCode::Char('k') | KeyCode::Up => self.switch_panel_up(),
                KeyCode::Char('w') => {
                    // Ctrl-W w cycles to next panel
                    self.switch_panel_down();
                }
                _ => {} // Ignore other keys after Ctrl-W
            }
            return;
        }

        // Check for Ctrl-W to start panel switching
        if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.waiting_for_panel_key = true;
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
            KeyCode::Char('j') | KeyCode::Down => {
                match self.active_panel {
                    ActivePanel::List => {
                        self.select_next();
                        // Update log viewer to show new job's logs
                        if self.show_log_viewer {
                            self.update_log_viewer_for_selected();
                        }
                    }
                    ActivePanel::Logs => {
                        // Scroll down in log viewer
                        if let Some(ref mut state) = self.log_viewer_state {
                            state.scroll_down();
                        }
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.active_panel {
                    ActivePanel::List => {
                        self.select_previous();
                        // Update log viewer to show new job's logs
                        if self.show_log_viewer {
                            self.update_log_viewer_for_selected();
                        }
                    }
                    ActivePanel::Logs => {
                        // Scroll up in log viewer
                        if let Some(ref mut state) = self.log_viewer_state {
                            state.scroll_up();
                        }
                    }
                }
            }
            KeyCode::Char('g') | KeyCode::Home => match self.active_panel {
                ActivePanel::List => self.select_first(),
                ActivePanel::Logs => {
                    if let Some(ref mut state) = self.log_viewer_state {
                        state.scroll_to_top();
                    }
                }
            },
            KeyCode::Char('G') | KeyCode::End => match self.active_panel {
                ActivePanel::List => self.select_last(),
                ActivePanel::Logs => {
                    if let Some(ref mut state) = self.log_viewer_state {
                        state.scroll_to_bottom();
                    }
                }
            },
            // Panel-specific keys
            KeyCode::Char('f') if self.active_panel == ActivePanel::List => self.cycle_filter(),
            KeyCode::Char('s') if self.active_panel == ActivePanel::List => self.cycle_sort(),
            KeyCode::Char('t') if self.active_panel == ActivePanel::Logs => {
                // Toggle tail/follow mode when logs panel is active
                if let Some(ref mut state) = self.log_viewer_state {
                    state.toggle_follow();
                }
            }
            // Page scrolling in logs (Ctrl-B/F or PageUp/PageDown)
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.active_panel == ActivePanel::Logs {
                    self.scroll_log_page_up();
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.active_panel == ActivePanel::Logs {
                    self.scroll_log_page_down();
                }
            }
            KeyCode::PageUp if self.active_panel == ActivePanel::Logs => {
                self.scroll_log_page_up();
            }
            KeyCode::PageDown if self.active_panel == ActivePanel::Logs => {
                self.scroll_log_page_down();
            }
            // Global keys
            KeyCode::Char('r') => self.toggle_view_mode(),
            KeyCode::Char('l') | KeyCode::Enter => self.toggle_log_viewer(),
            KeyCode::Char('?') => self.toggle_help(),
            KeyCode::Char('c') if self.active_panel == ActivePanel::List => self.copy_command(),
            KeyCode::Char('e') if self.active_panel == ActivePanel::List => {
                self.command_expanded = !self.command_expanded
            }
            KeyCode::Tab => {
                // Tab switches panels
                self.switch_panel_down();
            }
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
        let area = frame.area();

        // Adjust layout based on whether log panel is open
        let chunks = if self.show_log_viewer {
            // When logs are open:
            // - Main content (job list) caps at 45 lines (~40 jobs + headers)
            // - Log panel fills remaining space (minimum 12 lines)
            // Calculate heights: total = header(3) + main + logs + footer(1)
            let fixed_height = 3 + 1; // header + footer
            let available = area.height.saturating_sub(fixed_height as u16);

            // Cap main content at 45, give rest to logs (min 12)
            let main_height = available.saturating_sub(12).min(45);
            let log_height = available.saturating_sub(main_height);

            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),           // Header
                    Constraint::Length(main_height), // Main content (capped)
                    Constraint::Length(log_height),  // Log panel (fills rest)
                    Constraint::Length(1),           // Footer
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header (1 line + borders)
                    Constraint::Min(10),   // Main content
                    Constraint::Length(0), // No log panel
                    Constraint::Length(1), // Footer
                ])
                .split(area)
        };

        // Header
        Header::render(frame, chunks[0], &self.state);

        // Main content: split horizontally
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        // Render based on view mode - tabs are now in the block titles
        match self.view_mode {
            ViewMode::Jobs => {
                // Job list (left) and detail (right)
                let is_list_active = self.active_panel == ActivePanel::List;
                JobList::render(
                    frame,
                    main_chunks[0],
                    &self.state,
                    &self.job_ids,
                    Some(self.selected_index),
                    self.filter_mode.label(),
                    self.sort_mode.label(),
                    is_list_active,
                );

                // Render job detail or pipeline summary
                if self.is_main_pipeline_selected() {
                    JobDetail::render_pipeline(frame, main_chunks[1], &self.state);
                } else {
                    JobDetail::render(
                        frame,
                        main_chunks[1],
                        self.selected_job(),
                        self.command_expanded,
                    );
                }
            }
            ViewMode::Rules => {
                // Rule summary table (left panel)
                RuleSummary::render(
                    frame,
                    main_chunks[0],
                    &self.state,
                    &self.rule_names,
                    Some(self.selected_index),
                );

                // Show stats for selected rule in right panel
                if let Some(rule) = self.selected_rule() {
                    self.render_rule_detail(frame, main_chunks[1], rule);
                }
            }
        }

        // Log panel at bottom (if open)
        if self.show_log_viewer {
            self.render_log_panel(frame, chunks[2]);
        }

        // Get recent status message (within 3 seconds)
        let status_msg = self.status_message.as_ref().and_then(|(msg, timestamp)| {
            if timestamp.elapsed() < Duration::from_secs(3) {
                Some(msg.as_str())
            } else {
                None
            }
        });

        // Footer with optional status message
        Footer::render(frame, chunks[3], status_msg);

        // Help overlay (on top of everything)
        if self.show_help {
            self.render_help_overlay(frame);
        }
    }

    /// Render detail panel for selected rule.
    fn render_rule_detail(&self, frame: &mut Frame, area: Rect, rule: &str) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let mut lines = Vec::new();

        // Rule name
        lines.push(Line::from(vec![
            Span::styled("Rule: ", Style::default().fg(Color::Gray)),
            Span::styled(
                rule.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(""));

        // Get jobs for this rule
        let mut running = 0;
        let mut completed = 0;
        let mut failed = 0;
        let mut pending = 0;

        if let Some(job_ids) = self.state.jobs_by_rule.get(rule) {
            let mut total_runtime: u64 = 0;
            let mut completed_count = 0;

            for id in job_ids {
                if let Some(job) = self.state.jobs.get(id) {
                    match job.status {
                        JobStatus::Running => running += 1,
                        JobStatus::Completed => {
                            completed += 1;
                            if let (Some(start), Some(end)) =
                                (job.timing.started_at, job.timing.completed_at)
                            {
                                total_runtime += (end - start).num_seconds().max(0) as u64;
                                completed_count += 1;
                            }
                        }
                        JobStatus::Failed => failed += 1,
                        JobStatus::Pending | JobStatus::Queued => pending += 1,
                        _ => {}
                    }
                }
            }

            // Stats section
            lines.push(Line::from(Span::styled(
                "Statistics",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));

            lines.push(Line::from(vec![
                Span::styled("  Total: ", Style::default().fg(Color::Gray)),
                Span::styled(job_ids.len().to_string(), Style::default().fg(Color::White)),
            ]));

            lines.push(Line::from(vec![
                Span::styled("  Running: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    running.to_string(),
                    Style::default().fg(if running > 0 {
                        Color::Yellow
                    } else {
                        Color::Gray
                    }),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("  Completed: ", Style::default().fg(Color::Gray)),
                Span::styled(completed.to_string(), Style::default().fg(Color::Green)),
            ]));

            lines.push(Line::from(vec![
                Span::styled("  Failed: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    failed.to_string(),
                    Style::default().fg(if failed > 0 { Color::Red } else { Color::Gray }),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("  Pending: ", Style::default().fg(Color::Gray)),
                Span::styled(pending.to_string(), Style::default().fg(Color::Blue)),
            ]));

            // Timing section
            if completed_count > 0 {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Timing",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )));

                let avg_secs = total_runtime / completed_count as u64;
                lines.push(Line::from(vec![
                    Span::styled("  Avg runtime: ", Style::default().fg(Color::Gray)),
                    Span::styled(format_secs(avg_secs), Style::default().fg(Color::Yellow)),
                ]));

                lines.push(Line::from(vec![
                    Span::styled("  Total runtime: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format_secs(total_runtime),
                        Style::default().fg(Color::Green),
                    ),
                ]));
            }

            // Progress
            let progress = if !job_ids.is_empty() {
                completed * 100 / job_ids.len()
            } else {
                0
            };

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Progress: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}%", progress),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Add job status as colored text (compact display)
        let total = running + completed + failed + pending;
        if total > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Job Status",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));

            // Compact colored status: ▶3 ✓12 ✗1 ○5
            let mut status_spans = vec![Span::raw("  ")];

            if running > 0 {
                status_spans.push(Span::styled(
                    format!("▶{}", running),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                status_spans.push(Span::raw(" "));
            }

            status_spans.push(Span::styled(
                format!("✓{}", completed),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            status_spans.push(Span::raw(" "));

            if failed > 0 {
                status_spans.push(Span::styled(
                    format!("✗{}", failed),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
                status_spans.push(Span::raw(" "));
            }

            if pending > 0 {
                status_spans.push(Span::styled(
                    format!("○{}", pending),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            lines.push(Line::from(status_spans));
        }

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Rule Details "),
        );
        frame.render_widget(paragraph, area);
    }

    fn render_log_panel(&self, frame: &mut Frame, area: Rect) {
        if let Some(ref state) = self.log_viewer_state {
            // Render log viewer as a bottom panel (tailed output)
            let is_active = self.active_panel == ActivePanel::Logs;
            LogViewer::render_panel(frame, area, state, is_active);
        }
    }

    fn render_help_overlay(&self, frame: &mut Frame) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let area = centered_rect(60, 60, frame.area());

        let help_text = r#"
  Keyboard Shortcuts
  ──────────────────

  Panel Switching
  Tab        Switch active panel
  Ctrl-W j/k Switch to panel below/above

  Jobs Panel (when active)
  j/k / ↑↓   Navigate jobs
  g / G      Go to first/last job
  f          Cycle filter
  s          Cycle sort
  c          Copy command
  e          Expand/collapse command

  Logs Panel (when active)
  j/k / ↑↓   Scroll line by line
  Ctrl-B/F   Page up/down
  g / G      Go to top/bottom
  t          Toggle tail mode

  Global
  r          Toggle view (Jobs/Rules)
  l / Enter  Toggle log panel
  ?          This help
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

/// Format seconds as human-readable duration.
fn format_secs(secs: u64) -> String {
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
