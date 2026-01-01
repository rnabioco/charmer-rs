//! Log viewer component for displaying job log files.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use std::fs;
use std::io;
use std::path::Path;

/// State for the log viewer component.
#[derive(Debug, Clone)]
pub struct LogViewerState {
    /// Path to the log file being viewed
    pub log_path: String,
    /// Content lines from the log file
    pub lines: Vec<String>,
    /// Current scroll offset (0-indexed line number)
    pub scroll_offset: usize,
    /// Follow mode - auto-scroll to end
    pub follow_mode: bool,
    /// Error message if log couldn't be loaded
    pub error: Option<String>,
}

impl LogViewerState {
    /// Create a new log viewer state by loading the specified log file.
    pub fn new(log_path: String, tail_lines: usize) -> Self {
        let (lines, error) = match load_log_file(&log_path, tail_lines) {
            Ok(lines) => (lines, None),
            Err(e) => (vec![], Some(format!("Error loading log: {}", e))),
        };

        let scroll_offset = if !lines.is_empty() && tail_lines > 0 {
            lines.len().saturating_sub(1)
        } else {
            0
        };

        Self {
            log_path,
            lines,
            scroll_offset,
            follow_mode: false,
            error,
        }
    }

    /// Scroll down by one line.
    pub fn scroll_down(&mut self) {
        if !self.lines.is_empty() {
            self.scroll_offset = (self.scroll_offset + 1).min(self.lines.len().saturating_sub(1));
        }
        self.follow_mode = false;
    }

    /// Scroll up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        self.follow_mode = false;
    }

    /// Scroll to the top.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.follow_mode = false;
    }

    /// Scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        if !self.lines.is_empty() {
            self.scroll_offset = self.lines.len().saturating_sub(1);
        }
        self.follow_mode = false;
    }

    /// Toggle follow mode.
    pub fn toggle_follow(&mut self) {
        self.follow_mode = !self.follow_mode;
        if self.follow_mode {
            self.scroll_to_bottom();
        }
    }

    /// Get the visible lines for the given viewport height.
    pub fn visible_lines(&self, viewport_height: usize) -> &[String] {
        if self.lines.is_empty() {
            return &[];
        }

        let start = self.scroll_offset;
        let end = (start + viewport_height).min(self.lines.len());
        &self.lines[start..end]
    }

    /// Get scroll position information.
    pub fn scroll_info(&self) -> String {
        if self.lines.is_empty() {
            return "0/0".to_string();
        }
        format!("{}/{}", self.scroll_offset + 1, self.lines.len())
    }
}

/// Load log file contents, optionally tailing the last N lines.
fn load_log_file(path: &str, tail_lines: usize) -> io::Result<Vec<String>> {
    let path = Path::new(path);

    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Log file not found: {}", path.display()),
        ));
    }

    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines().map(String::from).collect();

    // If tail_lines is specified and we have more lines, take the last N
    if tail_lines > 0 && lines.len() > tail_lines {
        let skip_count = lines.len() - tail_lines;
        Ok(lines.into_iter().skip(skip_count).collect())
    } else {
        Ok(lines)
    }
}

/// Log viewer component.
pub struct LogViewer;

impl LogViewer {
    /// Render the log viewer component.
    pub fn render(frame: &mut Frame, area: Rect, state: &LogViewerState) {
        // Calculate content area (excluding borders)
        let content_height = area.height.saturating_sub(3); // Borders + footer

        // Build the title with the log file path
        let title = format!(" Log: {} ", state.log_path);

        // Get visible lines
        let visible_lines = state.visible_lines(content_height as usize);

        // Build content
        let content = if let Some(ref error) = state.error {
            // Show error message
            vec![Line::from(vec![Span::styled(
                error.clone(),
                Style::default().fg(Color::Red),
            )])]
        } else if visible_lines.is_empty() {
            // Show empty message
            vec![Line::from(vec![Span::styled(
                "Log file is empty",
                Style::default().fg(Color::DarkGray),
            )])]
        } else {
            // Show log lines
            visible_lines
                .iter()
                .map(|line| Line::from(vec![Span::raw(line.as_str())]))
                .collect()
        };

        // Build footer with scroll info and follow indicator
        let scroll_info = state.scroll_info();
        let follow_indicator = if state.follow_mode { " [follow]" } else { "" };
        let footer_text = format!(" {}{} ", scroll_info, follow_indicator);

        // Create the paragraph with borders
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_bottom(footer_text);

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);

        // Render scrollbar if content exceeds viewport
        if state.lines.len() > content_height as usize {
            let mut scrollbar_state =
                ScrollbarState::new(state.lines.len()).position(state.scroll_offset);

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    /// Render the log viewer footer with keybindings.
    pub fn render_footer(frame: &mut Frame, area: Rect) {
        let help = "j/k:scroll  g/G:top/bottom  F:follow  q/Esc:close";
        let paragraph = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }

    /// Render the log viewer as a bottom panel showing tailed output.
    pub fn render_panel(frame: &mut Frame, area: Rect, state: &LogViewerState) {
        // Calculate content area (excluding borders)
        let content_height = area.height.saturating_sub(2) as usize; // Top border + title

        // Build the title with log path and follow indicator
        let follow_indicator = if state.follow_mode { " [follow]" } else { "" };
        let title = format!(" Logs: {}{} ", state.log_path, follow_indicator);

        // Get the last N lines to show (tail view)
        let tail_lines = if state.lines.is_empty() {
            &[][..]
        } else {
            let start = state.lines.len().saturating_sub(content_height);
            &state.lines[start..]
        };

        // Build content
        let content: Vec<Line> = if let Some(ref error) = state.error {
            // Show error message
            vec![Line::from(vec![Span::styled(
                error.clone(),
                Style::default().fg(Color::Red),
            )])]
        } else if tail_lines.is_empty() {
            // Show empty message
            vec![Line::from(vec![Span::styled(
                "(waiting for log output...)",
                Style::default().fg(Color::DarkGray),
            )])]
        } else {
            // Show tailed log lines with syntax highlighting for common patterns
            tail_lines
                .iter()
                .map(|line| {
                    let style = if line.contains("ERROR") || line.contains("Error") {
                        Style::default().fg(Color::Red)
                    } else if line.contains("WARN") || line.contains("Warning") {
                        Style::default().fg(Color::Yellow)
                    } else if line.contains("INFO") || line.contains("rule ") {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(vec![Span::styled(line.as_str(), style)])
                })
                .collect()
        };

        // Create the block with border
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_style(Style::default().fg(Color::Cyan));

        let paragraph = Paragraph::new(content).block(block);

        frame.render_widget(paragraph, area);

        // Render scrollbar for panel if content exceeds viewport
        if state.lines.len() > content_height {
            let scroll_pos = state.lines.len().saturating_sub(content_height);
            let mut scrollbar_state = ScrollbarState::new(state.lines.len()).position(scroll_pos);

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }
}
