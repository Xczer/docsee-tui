use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

// ring buffer cap - drop oldest past this so memory stays flat
const MAX_LOG_LINES: usize = 2000;
const MAX_LINE_WIDTH: usize = 120;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub stream: LogStream,
    pub content: String,
    pub wrapped_lines: Vec<String>, // content already split to width so draw doesn't redo it
}

/// stdout vs stderr, we colour them differently
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

/// Live log tailer - handles word wrap, scrollback and the follow mode.
pub struct LogsViewer {
    docker_client: DockerClient,
    container: Option<Container>,
    log_entries: VecDeque<LogEntry>,
    display_lines: Vec<DisplayLine>, // flattened + wrapped, this is what actually gets drawn
    list_state: ListState,
    vertical_scrollbar_state: ScrollbarState,
    following: bool, // auto-scroll to bottom as new lines come in
    status_message: Option<String>,
    // background task streaming logs + the channel it pushes into
    log_handle: Option<tokio::task::JoinHandle<()>>,
    log_receiver: Option<mpsc::UnboundedReceiver<LogEntry>>,
    show_timestamps: bool,
    search_filter: Option<String>,
    word_wrap: bool,
    show_line_numbers: bool,
    auto_scroll_speed: usize,
    total_lines: usize,
    filtered_lines: usize,
}

// one row on screen after wrap/filter - a wrapped log line becomes several of these
#[derive(Debug, Clone)]
struct DisplayLine {
    content: String,
    stream: LogStream,
    timestamp: String,
    #[allow(dead_code)]
    original_index: usize,
    line_part: usize, // 0 = start of the line, 1+ = wrapped continuation
}

impl LogsViewer {
    /// empty viewer, no container attached till start_logs is called
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

    /// kick off streaming logs for a container, wiping whatever was showing before
    pub async fn start_logs(&mut self, container: Container) -> Result<()> {
        // kill the old stream first, warna do tasks push into the same channel
        self.stop_logs().await;

        self.container = Some(container.clone());
        self.log_entries.clear();
        self.display_lines.clear();
        self.following = true;
        self.list_state = ListState::default();
        self.vertical_scrollbar_state = ScrollbarState::default();

        // bg task pushes entries down this channel, update() drains it
        let (log_sender, log_receiver) = mpsc::unbounded_channel();
        self.log_receiver = Some(log_receiver);

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

    /// abort the streaming task and forget the container
    pub async fn stop_logs(&mut self) {
        if let Some(handle) = self.log_handle.take() {
            handle.abort();
        }
        self.log_receiver = None;
        self.container = None;
        self.status_message = None;
    }

    /// drain whatever the streaming task pushed, called every tick
    pub async fn update(&mut self) -> Result<()> {
        // try_recv in a loop, non-blocking, so we grab everything queued this tick
        let mut entries_to_add = Vec::new();
        if let Some(receiver) = &mut self.log_receiver {
            while let Ok(entry) = receiver.try_recv() {
                entries_to_add.push(entry);
            }
        }

        // only rebuild the display list if something actually came in
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
                // TODO in-log search - not wired up yet
                self.status_message = Some("Search inside logs - todo".to_string());
            }
            Key::Char('r') => self.refresh_display(),
            Key::Char('x') => self.export_logs_plaintext(),
            Key::Char('X') => self.export_logs_json(),
            _ => {}
        }

        Ok(())
    }

    /// Render header + log body.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // header strip
                Constraint::Min(0),    // log body + scrollbar
            ])
            .split(area);

        self.draw_enhanced_header(frame, chunks[0]);
        self.draw_logs_with_scrollbar(frame, chunks[1]);
    }

    /// Top strip - shows container name, follow state, line count etc.
    fn draw_enhanced_header(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("No Container");

        let status = if self.following {
            "🔄 Following"
        } else {
            "⏸️  Paused"
        };
        let timestamp_status = if self.show_timestamps {
            "🕐 With Timestamps"
        } else {
            "⏰ No Timestamps"
        };
        let wrap_status = if self.word_wrap {
            "↩️  Word Wrap"
        } else {
            "➡️ No Wrap"
        };
        let line_num_status = if self.show_line_numbers {
            "🔢 Line Numbers"
        } else {
            "📄 No Numbers"
        };

        let title = format!(
            "📋 Logs: {} | {} | {} | {} | {}",
            container_name, status, timestamp_status, wrap_status, line_num_status
        );

        let stats = format!(
            "Total: {} lines | Displayed: {} lines | Speed: {}x",
            self.total_lines, self.filtered_lines, self.auto_scroll_speed
        );

        let controls1 = "Navigation: ↑/↓ Scroll | PgUp/PgDn Page | Home/End Jump";
        let controls2 =
            "Features: f Follow | t Timestamps | w WordWrap | n LineNumbers | c Clear | +/- Speed";
        let controls3 = "Other: r Refresh | / Search | Esc Back";

        let title_text = if let Some(ref message) = self.status_message {
            format!("{} - {}", title, message)
        } else {
            title
        };

        if self.container.is_some() {
            let header_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
                .split(area);

            self.draw_container_info_panel(frame, header_chunks[0]);

            let header = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    title_text,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::styled("📊 ", Style::default().fg(Color::Yellow)),
                    Span::raw(stats),
                ]),
                Line::from(vec![
                    Span::styled("🎮 ", Style::default().fg(Color::Green)),
                    Span::raw(controls1),
                ]),
                Line::from(vec![Span::raw("   "), Span::raw(controls2)]),
                Line::from(vec![Span::raw("   "), Span::raw(controls3)]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: true });

            frame.render_widget(header, header_chunks[1]);
        } else {
            let header = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    title_text,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::styled("📊 ", Style::default().fg(Color::Yellow)),
                    Span::raw(stats),
                ]),
                Line::from(vec![
                    Span::styled("🎮 ", Style::default().fg(Color::Green)),
                    Span::raw(controls1),
                ]),
                Line::from(vec![Span::raw("   "), Span::raw(controls2)]),
                Line::from(vec![Span::raw("   "), Span::raw(controls3)]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: true });

            frame.render_widget(header, area);
        }
    }

    /// little name/image/status card on the left of the header
    fn draw_container_info_panel(&self, frame: &mut Frame, area: Rect) {
        let container = match &self.container {
            Some(c) => c,
            None => return,
        };

        let info_lines = vec![
            Line::from(vec![Span::styled(
                container.name.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("Image: ", Style::default().fg(Color::DarkGray)),
                Span::styled(container.image.clone(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(container.status.clone(), Style::default().fg(Color::Green)),
            ]),
        ];

        let panel = Paragraph::new(info_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Container ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(panel, area);
    }

    fn draw_logs_with_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        // shave one col off the right for the scrollbar to live in
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

        let filtered_lines: Vec<&DisplayLine> = if let Some(ref filter) = self.search_filter {
            self.display_lines
                .iter()
                .filter(|line| line.content.contains(filter))
                .collect()
        } else {
            self.display_lines.iter().collect()
        };

        self.filtered_lines = filtered_lines.len();

        // build the visible lines (timestamp colouring, wrap handled here)
        let items: Vec<ListItem> = filtered_lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let mut spans = Vec::new();

                if self.show_line_numbers {
                    spans.push(Span::styled(
                        format!("{:4} ", idx + 1),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                spans.push(Span::styled(
                    format!("{} ", line.stream.icon()),
                    Style::default().fg(line.stream.color()),
                ));

                if self.show_timestamps {
                    spans.push(Span::styled(
                        format!("[{}] ", line.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // little arrow so wrapped continuations don't look like separate log lines
                if line.line_part > 0 {
                    spans.push(Span::styled("↳ ", Style::default().fg(Color::DarkGray)));
                }

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
            if self.search_filter.is_some() {
                " filtered"
            } else {
                ""
            }
        );

        let logs_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(logs_title)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        // in follow mode keep pinning selection to the last line
        if self.following && !filtered_lines.is_empty() {
            let last_idx = filtered_lines.len().saturating_sub(1);
            self.list_state.select(Some(last_idx));
        }

        self.vertical_scrollbar_state = self
            .vertical_scrollbar_state
            .content_length(filtered_lines.len())
            .position(self.list_state.selected().unwrap_or(0));

        frame.render_stateful_widget(logs_list, logs_area, &mut self.list_state);

        // only bother with the scrollbar once content overflows the viewport
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

    /// push an entry into the ring buffer, pre-wrapping it if wrap is on
    fn add_log_entry(&mut self, mut entry: LogEntry) {
        if self.word_wrap {
            entry.wrapped_lines = self.wrap_text(&entry.content, MAX_LINE_WIDTH);
        } else {
            entry.wrapped_lines = vec![entry.content.clone()];
        }

        // drop the oldest once we hit the cap
        if self.log_entries.len() >= MAX_LOG_LINES {
            self.log_entries.pop_front();
        }

        self.log_entries.push_back(entry);
        self.total_lines += 1;
    }

    // dumb greedy word wrap, breaks on whitespace at the given width
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

    /// flatten every entry's wrapped lines back into the flat display list
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

    // any manual scroll drops out of follow mode
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

    fn scroll_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.display_lines.len().saturating_sub(1) {
                self.list_state.select(Some(selected + 1));
            } else {
                // hit the bottom, resume following
                self.following = true;
            }
        } else if !self.display_lines.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn page_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            let new_selected = selected.saturating_sub(10);
            self.list_state.select(Some(new_selected));
            self.following = false;
        }
    }

    fn page_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            let new_selected = (selected + 10).min(self.display_lines.len().saturating_sub(1));
            self.list_state.select(Some(new_selected));
            if new_selected == self.display_lines.len().saturating_sub(1) {
                self.following = true;
            }
        }
    }

    fn scroll_to_top(&mut self) {
        if !self.display_lines.is_empty() {
            self.list_state.select(Some(0));
            self.following = false;
        }
    }

    fn scroll_to_bottom(&mut self) {
        if !self.display_lines.is_empty() {
            self.list_state
                .select(Some(self.display_lines.len().saturating_sub(1)));
            self.following = true;
        }
    }

    fn toggle_follow(&mut self) {
        self.following = !self.following;
        if self.following && !self.display_lines.is_empty() {
            self.list_state
                .select(Some(self.display_lines.len().saturating_sub(1)));
        }
        let status = if self.following {
            "Following enabled"
        } else {
            "Following disabled"
        };
        self.status_message = Some(status.to_string());
    }

    fn toggle_timestamps(&mut self) {
        self.show_timestamps = !self.show_timestamps;
        let status = if self.show_timestamps {
            "Timestamps enabled"
        } else {
            "Timestamps disabled"
        };
        self.status_message = Some(status.to_string());
    }

    fn toggle_word_wrap(&mut self) {
        self.word_wrap = !self.word_wrap;
        // re-run every entry through add_log_entry so wrapped_lines gets redone
        let entries: Vec<LogEntry> = self.log_entries.iter().cloned().collect();
        self.log_entries.clear();
        for entry in entries {
            self.add_log_entry(entry);
        }
        self.rebuild_display();

        let status = if self.word_wrap {
            "Word wrap enabled"
        } else {
            "Word wrap disabled"
        };
        self.status_message = Some(status.to_string());
    }

    fn toggle_line_numbers(&mut self) {
        self.show_line_numbers = !self.show_line_numbers;
        let status = if self.show_line_numbers {
            "Line numbers enabled"
        } else {
            "Line numbers disabled"
        };
        self.status_message = Some(status.to_string());
    }

    fn increase_scroll_speed(&mut self) {
        self.auto_scroll_speed = (self.auto_scroll_speed + 1).min(10);
        self.status_message = Some(format!("Scroll speed: {}x", self.auto_scroll_speed));
    }

    fn decrease_scroll_speed(&mut self) {
        self.auto_scroll_speed = (self.auto_scroll_speed.saturating_sub(1)).max(1);
        self.status_message = Some(format!("Scroll speed: {}x", self.auto_scroll_speed));
    }

    fn refresh_display(&mut self) {
        self.rebuild_display();
        self.status_message = Some("Display refreshed".to_string());
    }

    fn clear_logs(&mut self) {
        self.log_entries.clear();
        self.display_lines.clear();
        self.list_state = ListState::default();
        self.total_lines = 0;
        self.filtered_lines = 0;
        self.status_message = Some("Logs cleared".to_string());
    }

    /// the bg task - tails the container over bollard and shoves entries down the channel
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
            tail: "100".to_string(), // don't drown on startup, just last 100
            ..Default::default()
        });

        let mut stream = docker_client.inner().logs(container_id, options);

        while let Some(log_output) = futures::stream::TryStreamExt::try_next(&mut stream).await? {
            use bollard::container::LogOutput;

            let (stream_type, content) = match log_output {
                LogOutput::StdOut { message } => (LogStream::Stdout, message),
                LogOutput::StdErr { message } => (LogStream::Stderr, message),
                LogOutput::Console { message } => (LogStream::Stdout, message),
                LogOutput::StdIn { .. } => continue,
            };

            // with timestamps=true docker prefixes each line "<rfc3339> <msg>", split on first space.
            // if there's no space fall back to local time
            let content_str = String::from_utf8_lossy(&content);
            let (timestamp, log_content) = if let Some(pos) = content_str.find(' ') {
                let (ts, content) = content_str.split_at(pos);
                (ts.to_string(), content.trim().to_string())
            } else {
                (
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    content_str.to_string(),
                )
            };

            let entry = LogEntry {
                timestamp,
                stream: stream_type,
                content: log_content,
                wrapped_lines: Vec::new(), // add_log_entry fills this
            };

            // send failing means the viewer went away, so bail out of the stream
            if sender.send(entry).is_err() {
                break;
            }
        }

        Ok(())
    }

    /// dump current buffer to ~/docsee-logs as a .txt
    fn export_logs_plaintext(&mut self) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("docsee-logs");

        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status_message = Some(format!("Failed to create dir: {}", e));
            return;
        }

        let filename = format!("{}_{}.txt", container_name, timestamp);
        let path = dir.join(&filename);

        let mut content = String::new();
        for entry in &self.log_entries {
            let stream_label = match entry.stream {
                LogStream::Stdout => "STDOUT",
                LogStream::Stderr => "STDERR",
            };
            content.push_str(&format!(
                "[{}] {} {}\n",
                entry.timestamp, stream_label, entry.content
            ));
        }

        match std::fs::write(&path, &content) {
            Ok(_) => {
                self.status_message = Some(format!(
                    "Exported {} lines to {}",
                    self.log_entries.len(),
                    path.display()
                ));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to write: {}", e));
            }
        }
    }

    /// same as plaintext export but structured json
    fn export_logs_json(&mut self) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("docsee-logs");

        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status_message = Some(format!("Failed to create dir: {}", e));
            return;
        }

        let filename = format!("{}_{}.json", container_name, timestamp);
        let path = dir.join(&filename);

        let entries: Vec<serde_json::Value> = self
            .log_entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "timestamp": entry.timestamp,
                    "stream": match entry.stream {
                        LogStream::Stdout => "stdout",
                        LogStream::Stderr => "stderr",
                    },
                    "content": entry.content,
                })
            })
            .collect();

        match serde_json::to_string_pretty(&entries) {
            Ok(json) => match std::fs::write(&path, &json) {
                Ok(_) => {
                    self.status_message = Some(format!(
                        "Exported {} lines to {}",
                        self.log_entries.len(),
                        path.display()
                    ));
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to write: {}", e));
                }
            },
            Err(e) => {
                self.status_message = Some(format!("JSON error: {}", e));
            }
        }
    }

    pub fn get_container(&self) -> Option<&Container> {
        self.container.as_ref()
    }
}
