//! Main TUI application.

use crate::components::{Footer, Header, JobDetail, JobList};
use crate::ui::Theme;
use charmer_state::{JobStatus, PipelineState};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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

    /// Update app state from external source (polling service).
    pub fn update_from_state(&mut self, new_state: PipelineState) {
        self.state = new_state;
        self.update_job_list();
    }

    /// Handle a key event.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('f') => self.cycle_filter(),
            KeyCode::Char('s') => self.cycle_sort(),
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header with progress
                Constraint::Length(1), // Status counts
                Constraint::Min(10),   // Main content
                Constraint::Length(1), // Footer
            ])
            .split(frame.area());

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
            Some(self.selected_index),
        );
        JobDetail::render(frame, main_chunks[1], self.selected_job());

        // Footer
        Footer::render(frame, chunks[3]);

        // Help overlay
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

    fn render_help_overlay(&self, frame: &mut Frame) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let area = centered_rect(60, 60, frame.area());

        let help_text = r#"
  Keyboard Shortcuts
  ──────────────────

  j / ↓      Move down
  k / ↑      Move up
  g / Home   Go to first job
  G / End    Go to last job
  f          Cycle filter (All/Running/Failed/Pending/Completed)
  s          Cycle sort (Status/Rule/Time)
  l / Enter  View logs (not yet implemented)
  ?          Toggle this help
  q / Ctrl+C Quit

  Press ? to close
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
