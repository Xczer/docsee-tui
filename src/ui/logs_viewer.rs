use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

/// Maximum number of log lines to keep in memory
const MAX_LOG_LINES: usize = 1000;

/// A single log entry with metadata
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub stream: LogStream,
    pub content: String,
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
}

/// The logs viewer widget
pub struct LogsViewer {
    /// Docker client for operations
    docker_client: DockerClient,
    /// Container being viewed
    container: Option<Container>,
    /// Log entries buffer
    log_entries: VecDeque<LogEntry>,
    /// List state for scrolling
    list_state: ListState,
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
}

impl LogsViewer {
    /// Create a new logs viewer
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            container: None,
            log_entries: VecDeque::with_capacity(MAX_LOG_LINES),
            list_state: ListState::default(),
            following: true,
            status_message: None,
            log_handle: None,
            log_receiver: None,
            show_timestamps: true,
            search_filter: None,
        }
    }

    /// Start viewing logs for a container
    pub async fn start_logs(&mut self, container: Container) -> Result<()> {
        // Stop any existing log stream
        self.stop_logs().await;

        self.container = Some(container.clone());
        self.log_entries.clear();
        self.following = true;
        self.list_state = ListState::default();

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
        
        // Add entries outside of the borrow
        for entry in entries_to_add {
            self.add_log_entry(entry);
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
            Key::Char('/') => {
                // TODO: Implement search
                self.status_message = Some("Search coming soon!".to_string());
            }
            _ => {}
        }

        Ok(())
    }

    /// Draw the logs viewer
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Split area into header and logs
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Logs
            ])
            .split(area);

        // Draw header
        self.draw_header(frame, chunks[0]);

        // Draw logs
        self.draw_logs(frame, chunks[1]);
    }

    /// Draw the header with container info and controls
    fn draw_header(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("No Container");

        let status = if self.following { "Following" } else { "Paused" };
        let timestamp_status = if self.show_timestamps { "With Timestamps" } else { "No Timestamps" };
        
        let title = format!(
            "Logs: {} | {} | {} | {} lines",
            container_name,
            status,
            timestamp_status,
            self.log_entries.len()
        );

        let title_text = if let Some(ref message) = self.status_message {
            format!("{} - {}", title, message)
        } else {
            title
        };

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::raw(title_text),
            ]),
            Line::from(vec![
                Span::styled("Controls: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("↑/↓ Scroll | PgUp/PgDn Page | Home/End | "),
                Span::styled("f", Style::default().fg(Color::Yellow)),
                Span::raw(" Follow | "),
                Span::styled("t", Style::default().fg(Color::Yellow)),
                Span::raw(" Timestamps | "),
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw(" Clear | "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" Back"),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(header, area);
    }

    /// Draw the logs list
    fn draw_logs(&mut self, frame: &mut Frame, area: Rect) {
        let filtered_entries: Vec<&LogEntry> = if let Some(ref filter) = self.search_filter {
            self.log_entries
                .iter()
                .filter(|entry| entry.content.contains(filter))
                .collect()
        } else {
            self.log_entries.iter().collect()
        };

        let items: Vec<ListItem> = filtered_entries
            .iter()
            .map(|entry| {
                let content = if self.show_timestamps {
                    format!("{} {}", entry.timestamp, entry.content)
                } else {
                    entry.content.clone()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(content, Style::default().fg(entry.stream.color()))
                ]))
            })
            .collect();

        let logs_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Log Output"))
            .highlight_style(Style::default().bg(Color::DarkGray));

        // Auto-scroll to bottom if following
        if self.following && !filtered_entries.is_empty() {
            self.list_state.select(Some(filtered_entries.len() - 1));
        }

        frame.render_stateful_widget(logs_list, area, &mut self.list_state);
    }

    /// Add a new log entry
    fn add_log_entry(&mut self, entry: LogEntry) {
        // Remove old entries if we're at capacity
        if self.log_entries.len() >= MAX_LOG_LINES {
            self.log_entries.pop_front();
        }

        self.log_entries.push_back(entry);
    }

    /// Scroll up one line
    fn scroll_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
                self.following = false;
            }
        } else if !self.log_entries.is_empty() {
            self.list_state.select(Some(self.log_entries.len() - 1));
            self.following = false;
        }
    }

    /// Scroll down one line
    fn scroll_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.log_entries.len() - 1 {
                self.list_state.select(Some(selected + 1));
            } else {
                self.following = true;
            }
        } else if !self.log_entries.is_empty() {
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
            let new_selected = (selected + 10).min(self.log_entries.len() - 1);
            self.list_state.select(Some(new_selected));
            if new_selected == self.log_entries.len() - 1 {
                self.following = true;
            }
        }
    }

    /// Scroll to top
    fn scroll_to_top(&mut self) {
        if !self.log_entries.is_empty() {
            self.list_state.select(Some(0));
            self.following = false;
        }
    }

    /// Scroll to bottom
    fn scroll_to_bottom(&mut self) {
        if !self.log_entries.is_empty() {
            self.list_state.select(Some(self.log_entries.len() - 1));
            self.following = true;
        }
    }

    /// Toggle follow mode
    fn toggle_follow(&mut self) {
        self.following = !self.following;
        if self.following && !self.log_entries.is_empty() {
            self.list_state.select(Some(self.log_entries.len() - 1));
        }
    }

    /// Toggle timestamp display
    fn toggle_timestamps(&mut self) {
        self.show_timestamps = !self.show_timestamps;
    }

    /// Clear all logs
    fn clear_logs(&mut self) {
        self.log_entries.clear();
        self.list_state = ListState::default();
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
