//! Log viewer component for displaying job log files.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
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

    /// Scroll to the bottom (disables follow mode since user is manually scrolling).
    pub fn scroll_to_bottom(&mut self) {
        if !self.lines.is_empty() {
            // Set offset to show the last page of content
            // This will be clamped during rendering based on viewport height
            self.scroll_offset = self.lines.len();
        }
        self.follow_mode = false;
    }

    /// Toggle follow mode.
    pub fn toggle_follow(&mut self) {
        self.follow_mode = !self.follow_mode;
        // Don't call scroll_to_bottom here as it would disable follow_mode
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
        let content_width = area.width.saturating_sub(3) as usize; // Borders + scrollbar

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
            // Show log lines with snakemake-aware syntax highlighting
            visible_lines
                .iter()
                .map(|line| Line::from(highlight_snakemake_line(line, content_width)))
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

        let paragraph = Paragraph::new(content).block(block);

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
    pub fn render_panel(frame: &mut Frame, area: Rect, state: &LogViewerState, is_active: bool) {
        // Calculate content area (excluding borders)
        let content_height = area.height.saturating_sub(2) as usize; // Top border + title
        let content_width = area.width.saturating_sub(3) as usize; // Left/right borders + scrollbar

        // Build the title with log path and follow indicator
        let follow_indicator = if state.follow_mode { " [tail]" } else { "" };
        let title = format!(" Logs: {}{} ", state.log_path, follow_indicator);
        let active_indicator = if is_active { " [active] " } else { "" };

        // Get lines to show based on scroll position
        // If in follow mode, show the tail; otherwise use scroll_offset
        let (visible_lines, visible_start) = if state.lines.is_empty() {
            (&[][..], 0)
        } else if state.follow_mode {
            // Tail view - show last N lines
            let start = state.lines.len().saturating_sub(content_height);
            (&state.lines[start..], start)
        } else {
            // Scrollable view - use scroll_offset
            // Clamp start so we always show a full viewport when possible
            let max_start = state.lines.len().saturating_sub(content_height);
            let start = state.scroll_offset.min(max_start);
            let end = (start + content_height).min(state.lines.len());
            (&state.lines[start..end], start)
        };

        // Build content
        let content: Vec<Line> = if let Some(ref error) = state.error {
            // Show error message
            vec![Line::from(vec![Span::styled(
                error.clone(),
                Style::default().fg(Color::Red),
            )])]
        } else if visible_lines.is_empty() {
            // Show empty message
            vec![Line::from(vec![Span::styled(
                "(waiting for log output...)",
                Style::default().fg(Color::DarkGray),
            )])]
        } else {
            // Show log lines with snakemake-aware syntax highlighting
            visible_lines
                .iter()
                .map(|line| Line::from(highlight_snakemake_line(line, content_width)))
                .collect()
        };

        // Build scroll position info for bottom title
        let scroll_info = if !state.lines.is_empty() {
            let display_start = visible_start + 1; // 1-indexed for display
            let display_end = (visible_start + visible_lines.len()).min(state.lines.len());
            format!(" {}-{}/{} ", display_start, display_end, state.lines.len())
        } else {
            String::new()
        };

        // Create the block with border - highlight when active
        let border_style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let title_style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        // Build title line with log path on left and active indicator on right
        let title_line = Line::from(vec![Span::styled(title, title_style)]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title_line)
            .title_top(Line::from(Span::styled(active_indicator, title_style)).right_aligned())
            .title_bottom(scroll_info);

        let paragraph = Paragraph::new(content).block(block);

        frame.render_widget(paragraph, area);

        // Render scrollbar for panel if content exceeds viewport
        if state.lines.len() > content_height {
            let scroll_pos = visible_start;
            let mut scrollbar_state = ScrollbarState::new(state.lines.len()).position(scroll_pos);

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            // Position scrollbar with inset from top and bottom (like job list)
            let scrollbar_area = Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height.saturating_sub(2), // 1 top + 1 bottom for borders
            };

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

/// Truncate a line to fit within the given width, accounting for unicode.
/// Returns an owned String to avoid lifetime issues.
fn truncate_line(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    // Strip ANSI escape sequences first (in case log has colors)
    let clean_line = strip_ansi(line);

    // Count grapheme clusters for proper unicode handling
    let char_count = clean_line.chars().count();
    if char_count <= max_width {
        clean_line
    } else if max_width <= 1 {
        "…".to_string()
    } else {
        // Take first (max_width - 1) characters and add ellipsis
        let truncated: String = clean_line.chars().take(max_width - 1).collect();
        format!("{}…", truncated)
    }
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                              // Skip until we hit a letter (end of sequence)
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if !c.is_control() || c == '\t' {
            // Keep normal characters and tabs, skip other control chars
            if c == '\t' {
                result.push_str("    "); // Replace tabs with spaces
            } else {
                result.push(c);
            }
        }
    }

    result
}

/// Highlight a snakemake log line with syntax-aware coloring.
/// Returns multiple spans for rich highlighting of different parts.
fn highlight_snakemake_line(line: &str, max_width: usize) -> Vec<Span<'static>> {
    let clean = truncate_line(line, max_width);
    let trimmed = clean.trim();

    // Timestamp: [Wed Mar 12 22:21:22 2025]
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 10 {
        return vec![Span::styled(clean, Style::default().fg(Color::DarkGray))];
    }

    // Rule header: localrule/checkpoint/rule NAME:
    if (trimmed.starts_with("localrule ")
        || trimmed.starts_with("checkpoint ")
        || trimmed.starts_with("rule "))
        && trimmed.ends_with(':')
    {
        return vec![Span::styled(
            clean,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
    }

    // Progress: X of Y steps (Z%) done
    if trimmed.contains(" of ") && trimmed.contains(" steps") && trimmed.contains("done") {
        return vec![Span::styled(clean, Style::default().fg(Color::Green))];
    }

    // Finished job
    if trimmed.starts_with("Finished job") {
        return vec![Span::styled(clean, Style::default().fg(Color::Green))];
    }

    // Error patterns
    if trimmed.starts_with("Error")
        || trimmed.contains("ERROR")
        || trimmed.contains("error:")
        || trimmed.contains("Exception")
        || trimmed.contains("exited with non-zero exit code")
    {
        return vec![Span::styled(clean, Style::default().fg(Color::Red))];
    }

    // Warning patterns
    if trimmed.contains("WARN") || trimmed.contains("Warning") {
        return vec![Span::styled(clean, Style::default().fg(Color::Yellow))];
    }

    // Field labels (input:, output:, jobid:, etc.)
    // These are indented with spaces
    if trimmed.starts_with("input:")
        || trimmed.starts_with("output:")
        || trimmed.starts_with("log:")
        || trimmed.starts_with("jobid:")
        || trimmed.starts_with("wildcards:")
        || trimmed.starts_with("resources:")
        || trimmed.starts_with("reason:")
        || trimmed.starts_with("benchmark:")
        || trimmed.starts_with("priority:")
        || trimmed.starts_with("threads:")
        || trimmed.starts_with("shell:")
        || trimmed.starts_with("message:")
        || trimmed.starts_with("conda-env:")
    {
        // Find the colon position
        if let Some(colon_pos) = trimmed.find(':') {
            let indent_len = clean.len() - clean.trim_start().len();
            let label = &trimmed[..=colon_pos]; // Include the colon
            let value = &trimmed[colon_pos + 1..];
            return vec![
                Span::raw(" ".repeat(indent_len)),
                Span::styled(label.to_string(), Style::default().fg(Color::Yellow)),
                Span::styled(value.to_string(), Style::default().fg(Color::White)),
            ];
        }
    }

    // Select/Execute jobs
    if trimmed.starts_with("Select") || trimmed.starts_with("Execute") {
        return vec![Span::styled(clean, Style::default().fg(Color::Blue))];
    }

    // Linting
    if trimmed.starts_with("Lints for") {
        return vec![Span::styled(clean, Style::default().fg(Color::Magenta))];
    }

    // Job stats header
    if trimmed == "Job stats:" || trimmed.starts_with("job ") && trimmed.contains("count") {
        return vec![Span::styled(
            clean,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )];
    }

    // Building DAG
    if trimmed.starts_with("Building DAG") {
        return vec![Span::styled(clean, Style::default().fg(Color::Cyan))];
    }

    // Host/cores info
    if trimmed.starts_with("host:") || trimmed.starts_with("Provided cores:") {
        return vec![Span::styled(clean, Style::default().fg(Color::Gray))];
    }

    // Write-protecting output
    if trimmed.starts_with("Write-protecting") {
        return vec![Span::styled(clean, Style::default().fg(Color::DarkGray))];
    }

    // Default: white
    vec![Span::styled(clean, Style::default().fg(Color::White))]
}
