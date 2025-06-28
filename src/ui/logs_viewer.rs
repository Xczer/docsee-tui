use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

/// Maximum number of log lines to keep in memory
const MAX_LOG_LINES: usize = 2000;
/// Maximum line width before wrapping
const MAX_LINE_WIDTH: usize = 120;

/// A single log entry with metadata
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub stream: LogStream,
    pub content: String,
    pub wrapped_lines: Vec<String>, // Pre-wrapped content for display
}

/// Which stream the log entry came from
#[derive(Debug, Clone, PartialEq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

impl LogStream {
    pub fn color(&self) -> Color {
        match self {
            LogStream::Stdout => Color::White,
            LogStream::Stderr => Color::Red,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            LogStream::Stdout => "📄",
            LogStream::Stderr => "❌",
        }
    }
}

/// Enhanced logs viewer widget with word wrap and scrollbars
pub struct LogsViewer {
    /// Docker client for operations
    docker_client: DockerClient,
    /// Container being viewed
    container: Option<Container>,
    /// Log entries buffer
    log_entries: VecDeque<LogEntry>,
    /// Flattened display lines (after word wrapping)
    display_lines: Vec<DisplayLine>,
    /// List state for scrolling
    list_state: ListState,
    /// Vertical scrollbar state
    vertical_scrollbar_state: ScrollbarState,
    /// Whether we're following the logs (auto-scroll)
    following: bool,
    /// Status message
    status_message: Option<String>,
    /// Log streaming handle
    log_handle: Option<tokio::task::JoinHandle<()>>,
    /// Channel for receiving log entries
    log_receiver: Option<mpsc::UnboundedReceiver<LogEntry>>,
    /// Whether to show timestamps
    show_timestamps: bool,
    /// Search filter
    search_filter: Option<String>,
    /// Line wrapping enabled
    word_wrap: bool,
    /// Show line numbers
    show_line_numbers: bool,
    /// Auto-scroll speed (lines per tick)
    auto_scroll_speed: usize,
    /// Display statistics
    total_lines: usize,
    filtered_lines: usize,
}

/// A display line after processing (wrapping, filtering, etc.)
#[derive(Debug, Clone)]
struct DisplayLine {
    content: String,
    stream: LogStream,
    timestamp: String,
    original_index: usize,
    line_part: usize, // For wrapped lines: 0 = first part, 1+ = continuation
}

impl LogsViewer {
    /// Create a new enhanced logs viewer
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            container: None,
            log_entries: VecDeque::with_capacity(MAX_LOG_LINES),
            display_lines: Vec::new(),
            list_state: ListState::default(),
            vertical_scrollbar_state: ScrollbarState::default(),
            following: true,
            status_message: None,
            log_handle: None,
            log_receiver: None,
            show_timestamps: true,
            search_filter: None,
            word_wrap: true,
            show_line_numbers: false,
            auto_scroll_speed: 1,
            total_lines: 0,
            filtered_lines: 0,
        }
    }

    /// Start viewing logs for a container
    pub async fn start_logs(&mut self, container: Container) -> Result<()> {
        // Stop any existing log stream
        self.stop_logs().await;

        self.container = Some(container.clone());
        self.log_entries.clear();
        self.display_lines.clear();
        self.following = true;
        self.list_state = ListState::default();
        self.vertical_scrollbar_state = ScrollbarState::default();

        // Create channel for log entries
        let (log_sender, log_receiver) = mpsc::unbounded_channel();
        self.log_receiver = Some(log_receiver);

        // Start log streaming task
        let docker_client = self.docker_client.clone();
        let container_id = container.id.clone();
        let container_name = container.name.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::stream_logs(docker_client, &container_id, log_sender).await {
                eprintln!("Error streaming logs for {}: {}", container_name, e);
            }
        });

        self.log_handle = Some(handle);
        self.status_message = Some(format!("Streaming logs for '{}'...", container.name));

        Ok(())
    }

    /// Stop log streaming
    pub async fn stop_logs(&mut self) {
        if let Some(handle) = self.log_handle.take() {
            handle.abort();
        }
        self.log_receiver = None;
        self.container = None;
        self.status_message = None;
    }

    /// Update the logs viewer (call this in your main event loop)
    pub async fn update(&mut self) -> Result<()> {
        // Process incoming log entries
        let mut entries_to_add = Vec::new();
        if let Some(receiver) = &mut self.log_receiver {
            while let Ok(entry) = receiver.try_recv() {
                entries_to_add.push(entry);
            }
        }
        
        // Add entries and rebuild display
        let mut needs_rebuild = false;
        for entry in entries_to_add {
            self.add_log_entry(entry);
            needs_rebuild = true;
        }

        if needs_rebuild {
            self.rebuild_display();
        }

        Ok(())
    }

    /// Handle key events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Up => self.scroll_up(),
            Key::Down => self.scroll_down(),
            Key::PageUp => self.page_up(),
            Key::PageDown => self.page_down(),
            Key::Home => self.scroll_to_top(),
            Key::End => self.scroll_to_bottom(),
            Key::Char('f') => self.toggle_follow(),
            Key::Char('t') => self.toggle_timestamps(),
            Key::Char('c') => self.clear_logs(),
            Key::Char('w') => self.toggle_word_wrap(),
            Key::Char('n') => self.toggle_line_numbers(),
            Key::Char('+') => self.increase_scroll_speed(),
            Key::Char('-') => self.decrease_scroll_speed(),
            Key::Char('/') => {
                // TODO: Implement search
                self.status_message = Some("Search: Type to filter logs (coming soon!)".to_string());
            }
            Key::Char('r') => self.refresh_display(),
            _ => {}
        }

        Ok(())
    }

    /// Draw the enhanced logs viewer
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Split area into header and logs
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Enhanced header
                Constraint::Min(0),    // Logs with scrollbar
            ])
            .split(area);

        // Draw enhanced header
        self.draw_enhanced_header(frame, chunks[0]);

        // Draw logs with scrollbar
        self.draw_logs_with_scrollbar(frame, chunks[1]);
    }

    /// Draw the enhanced header with more information
    fn draw_enhanced_header(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("No Container");

        let status = if self.following { "🔄 Following" } else { "⏸️  Paused" };
        let timestamp_status = if self.show_timestamps { "🕐 With Timestamps" } else { "⏰ No Timestamps" };
        let wrap_status = if self.word_wrap { "↩️  Word Wrap" } else { "➡️ No Wrap" };
        let line_num_status = if self.show_line_numbers { "🔢 Line Numbers" } else { "📄 No Numbers" };
        
        let title = format!(
            "📋 Logs: {} | {} | {} | {} | {}",
            container_name,
            status,
            timestamp_status,
            wrap_status,
            line_num_status
        );

        let stats = format!(
            "Total: {} lines | Displayed: {} lines | Speed: {}x",
            self.total_lines,
            self.filtered_lines,
            self.auto_scroll_speed
        );

        let controls1 = "Navigation: ↑/↓ Scroll | PgUp/PgDn Page | Home/End Jump";
        let controls2 = "Features: f Follow | t Timestamps | w WordWrap | n LineNumbers | c Clear | +/- Speed";
        let controls3 = "Other: r Refresh | / Search | Esc Back";

        let title_text = if let Some(ref message) = self.status_message {
            format!("{} - {}", title, message)
        } else {
            title
        };

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(title_text, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("📊 ", Style::default().fg(Color::Yellow)),
                Span::raw(stats),
            ]),
            Line::from(vec![
                Span::styled("🎮 ", Style::default().fg(Color::Green)),
                Span::raw(controls1),
            ]),
            Line::from(vec![
                Span::raw("   "),
                Span::raw(controls2),
            ]),
            Line::from(vec![
                Span::raw("   "),
                Span::raw(controls3),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
        .wrap(Wrap { trim: true });

        frame.render_widget(header, area);
    }

    /// Draw the logs list with scrollbar
    fn draw_logs_with_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        // Reserve space for vertical scrollbar
        let logs_area = Rect {
            width: area.width.saturating_sub(1),
            ..area
        };
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };

        // Apply search filter
        let filtered_lines: Vec<&DisplayLine> = if let Some(ref filter) = self.search_filter {
            self.display_lines
                .iter()
                .filter(|line| line.content.contains(filter))
                .collect()
        } else {
            self.display_lines.iter().collect()
        };

        self.filtered_lines = filtered_lines.len();

        // Create list items with enhanced formatting
        let items: Vec<ListItem> = filtered_lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let mut spans = Vec::new();

                // Add line number if enabled
                if self.show_line_numbers {
                    spans.push(Span::styled(
                        format!("{:4} ", idx + 1),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // Add stream icon
                spans.push(Span::styled(
                    format!("{} ", line.stream.icon()),
                    Style::default().fg(line.stream.color()),
                ));

                // Add timestamp if enabled
                if self.show_timestamps {
                    spans.push(Span::styled(
                        format!("[{}] ", line.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // Add continuation indicator for wrapped lines
                if line.line_part > 0 {
                    spans.push(Span::styled(
                        "↳ ",
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // Add main content
                spans.push(Span::styled(
                    line.content.clone(),
                    Style::default().fg(line.stream.color()),
                ));

                ListItem::new(Line::from(spans))
            })
            .collect();

        let logs_title = format!(
            "Log Output ({} lines{})",
            filtered_lines.len(),
            if self.search_filter.is_some() { " filtered" } else { "" }
        );

        let logs_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(logs_title)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            );

        // Auto-scroll to bottom if following
        if self.following && !filtered_lines.is_empty() {
            let last_idx = filtered_lines.len().saturating_sub(1);
            self.list_state.select(Some(last_idx));
        }

        // Update scrollbar state
        self.vertical_scrollbar_state = self.vertical_scrollbar_state
            .content_length(filtered_lines.len())
            .position(self.list_state.selected().unwrap_or(0));

        // Render list and scrollbar
        frame.render_stateful_widget(logs_list, logs_area, &mut self.list_state);
        
        if filtered_lines.len() > area.height as usize {
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .track_symbol(Some("│"))
                    .thumb_symbol("█"),
                scrollbar_area,
                &mut self.vertical_scrollbar_state,
            );
        }
    }

    /// Add a new log entry with word wrapping
    fn add_log_entry(&mut self, mut entry: LogEntry) {
        // Wrap content if needed
        if self.word_wrap {
            entry.wrapped_lines = self.wrap_text(&entry.content, MAX_LINE_WIDTH);
        } else {
            entry.wrapped_lines = vec![entry.content.clone()];
        }

        // Remove old entries if we're at capacity
        if self.log_entries.len() >= MAX_LOG_LINES {
            self.log_entries.pop_front();
        }

        self.log_entries.push_back(entry);
        self.total_lines += 1;
    }

    /// Wrap text to specified width
    fn wrap_text(&self, text: &str, width: usize) -> Vec<String> {
        if text.len() <= width {
            return vec![text.to_string()];
        }

        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0;

        for word in text.split_whitespace() {
            let word_len = word.len();
            
            if current_width + word_len + 1 > width && !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line.clear();
                current_width = 0;
            }
            
            if !current_line.is_empty() {
                current_line.push(' ');
                current_width += 1;
            }
            
            current_line.push_str(word);
            current_width += word_len;
        }
        
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }
        
        lines
    }

    /// Rebuild the display lines from log entries
    fn rebuild_display(&mut self) {
        self.display_lines.clear();

        for (entry_idx, entry) in self.log_entries.iter().enumerate() {
            for (line_part, wrapped_line) in entry.wrapped_lines.iter().enumerate() {
                self.display_lines.push(DisplayLine {
                    content: wrapped_line.clone(),
                    stream: entry.stream.clone(),
                    timestamp: entry.timestamp.clone(),
                    original_index: entry_idx,
                    line_part,
                });
            }
        }
    }

    /// Scroll up one line
    fn scroll_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
                self.following = false;
            }
        } else if !self.display_lines.is_empty() {
            self.list_state.select(Some(self.display_lines.len() - 1));
            self.following = false;
        }
    }

    /// Scroll down one line
    fn scroll_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.display_lines.len().saturating_sub(1) {
                self.list_state.select(Some(selected + 1));
            } else {
                self.following = true;
            }
        } else if !self.display_lines.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Scroll up one page
    fn page_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            let new_selected = selected.saturating_sub(10);
            self.list_state.select(Some(new_selected));
            self.following = false;
        }
    }

    /// Scroll down one page
    fn page_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            let new_selected = (selected + 10).min(self.display_lines.len().saturating_sub(1));
            self.list_state.select(Some(new_selected));
            if new_selected == self.display_lines.len().saturating_sub(1) {
                self.following = true;
            }
        }
    }

    /// Scroll to top
    fn scroll_to_top(&mut self) {
        if !self.display_lines.is_empty() {
            self.list_state.select(Some(0));
            self.following = false;
        }
    }

    /// Scroll to bottom
    fn scroll_to_bottom(&mut self) {
        if !self.display_lines.is_empty() {
            self.list_state.select(Some(self.display_lines.len().saturating_sub(1)));
            self.following = true;
        }
    }

    /// Toggle follow mode
    fn toggle_follow(&mut self) {
        self.following = !self.following;
        if self.following && !self.display_lines.is_empty() {
            self.list_state.select(Some(self.display_lines.len().saturating_sub(1)));
        }
        let status = if self.following { "Following enabled" } else { "Following disabled" };
        self.status_message = Some(status.to_string());
    }

    /// Toggle timestamp display
    fn toggle_timestamps(&mut self) {
        self.show_timestamps = !self.show_timestamps;
        let status = if self.show_timestamps { "Timestamps enabled" } else { "Timestamps disabled" };
        self.status_message = Some(status.to_string());
    }

    /// Toggle word wrap
    fn toggle_word_wrap(&mut self) {
        self.word_wrap = !self.word_wrap;
        // Rebuild all entries with new wrap setting
        let entries: Vec<LogEntry> = self.log_entries.iter().cloned().collect();
        self.log_entries.clear();
        for entry in entries {
            self.add_log_entry(entry);
        }
        self.rebuild_display();
        
        let status = if self.word_wrap { "Word wrap enabled" } else { "Word wrap disabled" };
        self.status_message = Some(status.to_string());
    }

    /// Toggle line numbers
    fn toggle_line_numbers(&mut self) {
        self.show_line_numbers = !self.show_line_numbers;
        let status = if self.show_line_numbers { "Line numbers enabled" } else { "Line numbers disabled" };
        self.status_message = Some(status.to_string());
    }

    /// Increase auto-scroll speed
    fn increase_scroll_speed(&mut self) {
        self.auto_scroll_speed = (self.auto_scroll_speed + 1).min(10);
        self.status_message = Some(format!("Scroll speed: {}x", self.auto_scroll_speed));
    }

    /// Decrease auto-scroll speed
    fn decrease_scroll_speed(&mut self) {
        self.auto_scroll_speed = (self.auto_scroll_speed.saturating_sub(1)).max(1);
        self.status_message = Some(format!("Scroll speed: {}x", self.auto_scroll_speed));
    }

    /// Refresh display
    fn refresh_display(&mut self) {
        self.rebuild_display();
        self.status_message = Some("Display refreshed".to_string());
    }

    /// Clear all logs
    fn clear_logs(&mut self) {
        self.log_entries.clear();
        self.display_lines.clear();
        self.list_state = ListState::default();
        self.total_lines = 0;
        self.filtered_lines = 0;
        self.status_message = Some("Logs cleared".to_string());
    }

    /// Stream logs from Docker
    async fn stream_logs(
        docker_client: DockerClient,
        container_id: &str,
        sender: mpsc::UnboundedSender<LogEntry>,
    ) -> Result<()> {
        use bollard::container::LogsOptions;
        use chrono::Local;

        let options = Some(LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            timestamps: true,
            tail: "100".to_string(), // Start with last 100 lines
            ..Default::default()
        });

        let mut stream = docker_client.inner().logs(container_id, options);

        while let Some(log_output) = futures::stream::TryStreamExt::try_next(&mut stream).await? {
            use bollard::container::LogOutput;

            let (stream_type, content) = match log_output {
                LogOutput::StdOut { message } => (LogStream::Stdout, message),
                LogOutput::StdErr { message } => (LogStream::Stderr, message),
                LogOutput::Console { message } => (LogStream::Stdout, message),
                LogOutput::StdIn { .. } => continue, // Skip stdin
            };

            // Parse the log content (Docker includes timestamps)
            let content_str = String::from_utf8_lossy(&content);
            let (timestamp, log_content) = if let Some(pos) = content_str.find(' ') {
                let (ts, content) = content_str.split_at(pos);
                (ts.to_string(), content.trim().to_string())
            } else {
                (Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), content_str.to_string())
            };

            let entry = LogEntry {
                timestamp,
                stream: stream_type,
                content: log_content,
                wrapped_lines: Vec::new(), // Will be populated in add_log_entry
            };

            // Send the log entry to the UI
            if sender.send(entry).is_err() {
                // Receiver has been dropped, stop streaming
                break;
            }
        }

        Ok(())
    }

    /// Get the current container
    pub fn get_container(&self) -> Option<&Container> {
        self.container.as_ref()
    }
}
