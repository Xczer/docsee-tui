use anyhow::{Context, Result};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::{
    collections::VecDeque,
    io::{self, Write},
    process::{Command, Stdio},
};

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    Normal, // arrow keys scroll output / history
    Typing, // every key goes into the input buffer
}

/// One shell session against a container
pub struct ShellExecutor {
    _docker_client: DockerClient,
    container: Option<Container>,
    command_history: VecDeque<String>,
    current_input: String,
    cursor_position: usize,
    history_position: Option<usize>,
    status_message: Option<String>,
    available_shells: Vec<String>,
    current_shell: usize,
    input_mode: InputMode,
    output_history: VecDeque<String>,
    output_list_state: ListState,
    show_help: bool,
}

impl ShellExecutor {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            _docker_client: docker_client,
            container: None,
            command_history: VecDeque::with_capacity(MAX_HISTORY),
            current_input: String::new(),
            cursor_position: 0,
            history_position: None,
            status_message: None,
            available_shells: vec![
                "/bin/bash".to_string(),
                "/bin/sh".to_string(),
                "/bin/zsh".to_string(),
                "/bin/fish".to_string(),
                "sh".to_string(),
            ],
            current_shell: 0,
            input_mode: InputMode::Typing, // start in typing, feels more natural
            output_history: VecDeque::with_capacity(200),
            output_list_state: ListState::default(),
            show_help: false,
        }
    }

    /// point this session at a container and reset the input state
    pub fn set_container(&mut self, container: Container) {
        self.container = Some(container);
        self.current_input.clear();
        self.cursor_position = 0;
        self.history_position = None;
        self.input_mode = InputMode::Typing;
        self.output_history.clear();
        self.status_message = Some(format!(
            "Ready to execute commands in '{}'",
            self.container.as_ref().unwrap().name
        ));

        self.add_output_line(
            "🐚 Shell session started. Type 'help' for commands, 'exit' to return.".to_string(),
        );
        self.add_output_line("💡 Press F1 to toggle input mode, Tab to switch shells.".to_string());
    }

    /// entry point for every key. esc/f1/f2 are global, rest depends on mode
    pub async fn handle_key(&mut self, key: Key) -> Result<bool> {
        match key {
            Key::Esc => {
                return Ok(true); // true means "leave shell mode"
            }
            Key::F1 => {
                self.toggle_input_mode();
                return Ok(false);
            }
            Key::F2 => {
                self.show_help = !self.show_help;
                return Ok(false);
            }

            _ => match self.input_mode {
                InputMode::Typing => self.handle_typing_mode(key).await?,
                InputMode::Normal => self.handle_normal_mode(key).await?,
            },
        }

        Ok(false)
    }

    async fn handle_typing_mode(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Enter => {
                self.execute_command().await?;
            }
            Key::Char(c) => {
                self.current_input.insert(self.cursor_position, c);
                self.cursor_position += 1;
                self.history_position = None;
            }
            Key::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.current_input.remove(self.cursor_position);
                    self.history_position = None;
                }
            }
            Key::Delete => {
                if self.cursor_position < self.current_input.len() {
                    self.current_input.remove(self.cursor_position);
                }
            }
            Key::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            Key::Right => {
                if self.cursor_position < self.current_input.len() {
                    self.cursor_position += 1;
                }
            }
            Key::Home => {
                self.cursor_position = 0;
            }
            Key::End => {
                self.cursor_position = self.current_input.len();
            }
            Key::Up => {
                self.navigate_history_up();
            }
            Key::Down => {
                self.navigate_history_down();
            }
            Key::Tab => {
                self.cycle_shell();
            }
            Key::Ctrl('c') => {
                self.current_input.clear();
                self.cursor_position = 0;
                self.history_position = None;
                self.add_output_line("^C".to_string());
            }
            Key::Ctrl('l') => {
                self.output_history.clear();
                self.add_output_line("Shell cleared".to_string());
            }
            Key::Ctrl('u') => {
                self.current_input.drain(0..self.cursor_position);
                self.cursor_position = 0;
            }
            Key::Ctrl('k') => {
                self.current_input.truncate(self.cursor_position);
            }
            Key::Ctrl('a') => {
                self.cursor_position = 0;
            }
            Key::Ctrl('e') => {
                self.cursor_position = self.current_input.len();
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_normal_mode(&mut self, key: Key) -> Result<()> {
        match key {
            Key::Up => {
                self.scroll_output_up();
            }
            Key::Down => {
                self.scroll_output_down();
            }
            Key::PageUp => {
                self.page_output_up();
            }
            Key::PageDown => {
                self.page_output_down();
            }
            Key::Home => {
                self.scroll_output_to_top();
            }
            Key::End => {
                self.scroll_output_to_bottom();
            }
            Key::Char('c') => {
                self.output_history.clear();
                self.add_output_line("Output cleared".to_string());
            }
            Key::Tab => {
                self.cycle_shell();
            }
            _ => {}
        }

        Ok(())
    }

    fn toggle_input_mode(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Typing => InputMode::Normal,
            InputMode::Normal => InputMode::Typing,
        };

        let mode_name = match self.input_mode {
            InputMode::Typing => "Typing Mode - All keys go to input",
            InputMode::Normal => "Navigation Mode - Arrow keys scroll output",
        };

        self.status_message = Some(format!("Switched to {}", mode_name));
    }

    /// draws the shell pane - header on top, output in middle, prompt at bottom
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(if self.show_help { 8 } else { 4 }), // header grows when help is on
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        self.draw_enhanced_header(frame, chunks[0]);
        self.draw_output_history(frame, chunks[1]);
        self.draw_enhanced_input(frame, chunks[2]);
    }

    /// header showing which shell + input mode we are in
    fn draw_enhanced_header(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("No Container");

        let current_shell = &self.available_shells[self.current_shell];
        let mode_indicator = match self.input_mode {
            InputMode::Typing => "⌨️  TYPING",
            InputMode::Normal => "🧭 NAVIGATE",
        };

        let title = format!(
            "🐚 Shell: {} ({}) | {}",
            container_name, current_shell, mode_indicator
        );

        let title_text = if let Some(ref message) = self.status_message {
            format!("{} - {}", title, message)
        } else {
            title
        };

        let mut header_lines = vec![
            Line::from(vec![Span::styled(
                title_text,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("🎮 Controls: ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Esc",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Exit | "),
                Span::styled(
                    "F1",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Toggle Mode | "),
                Span::styled(
                    "Tab",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Switch Shell | "),
                Span::styled(
                    "F2",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Help"),
            ]),
        ];

        if self.show_help {
            header_lines.extend(vec![
                Line::from(vec![
                    Span::styled(
                        "💡 Typing Mode: ",
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Enter execute | ↑/↓ history | Ctrl+C clear | Ctrl+L clear output"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "🧭 Navigate Mode: ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("↑/↓ scroll output | PgUp/PgDn page | Home/End jump | c clear"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "🔧 Advanced: ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Ctrl+A/E line start/end | Ctrl+U/K clear line left/right"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "⚠️  Warning: ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Commands execute directly in container! Use with caution."),
                ]),
            ]);
        }

        let header = Paragraph::new(header_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue)),
        );

        frame.render_widget(header, area);
    }

    fn draw_output_history(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .output_history
            .iter()
            .enumerate()
            .map(|(i, output)| {
                let line_number = format!("{:3} ", i + 1);
                ListItem::new(Line::from(vec![
                    Span::styled(line_number, Style::default().fg(Color::DarkGray)),
                    Span::raw(output.clone()),
                ]))
            })
            .collect();

        let output_title = format!(
            "Command Output ({} lines) - Mode: {}",
            self.output_history.len(),
            match self.input_mode {
                InputMode::Typing => "Input focus",
                InputMode::Normal => "Navigation focus",
            }
        );

        let output_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(output_title)
                    .border_style(match self.input_mode {
                        InputMode::Normal => Style::default().fg(Color::Yellow), // yellow border = nav mode is active
                        InputMode::Typing => Style::default().fg(Color::Gray),
                    }),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        // in typing mode keep pinned to the last line, warna user ko latest output dikhta nahi
        if self.input_mode == InputMode::Typing && !self.output_history.is_empty() {
            self.output_list_state
                .select(Some(self.output_history.len() - 1));
        }

        frame.render_stateful_widget(output_list, area, &mut self.output_list_state);
    }

    /// the prompt line at the bottom, we draw our own block cursor here
    fn draw_enhanced_input(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("container");

        let prompt = format!("{}@{}:~$ ", "user", container_name);

        let mut input_spans = vec![Span::styled(
            prompt,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )];

        // three cases - cursor at start, at end, or somewhere in between
        if self.cursor_position == 0 {
            input_spans.push(Span::styled(
                "█",
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ));
            input_spans.push(Span::raw(&self.current_input));
        } else if self.cursor_position >= self.current_input.len() {
            input_spans.push(Span::raw(&self.current_input));
            input_spans.push(Span::styled(
                "█",
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ));
        } else {
            let (before, after) = self.current_input.split_at(self.cursor_position);
            let cursor_char = after.chars().next().unwrap_or(' ');
            // step past the char under cursor by its utf8 width, not just +1
            let after_cursor = &after[cursor_char.len_utf8()..];

            input_spans.push(Span::raw(before));
            input_spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            ));
            input_spans.push(Span::raw(after_cursor));
        }

        let input_line = Paragraph::new(Line::from(input_spans)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Command Input")
                .border_style(match self.input_mode {
                    InputMode::Typing => Style::default().fg(Color::Yellow),
                    InputMode::Normal => Style::default().fg(Color::Gray),
                }),
        );

        frame.render_widget(input_line, area);
    }

    async fn execute_command(&mut self) -> Result<()> {
        if self.current_input.trim().is_empty() {
            return Ok(());
        }

        let command = self.current_input.clone();
        self.add_to_history(command.clone());

        // echo the command back so the output pane reads like a real terminal
        self.add_output_line(format!("$ {}", command));

        self.current_input.clear();
        self.cursor_position = 0;
        self.history_position = None;

        // few commands we handle ourselves before hitting docker exec
        match command.trim() {
            "exit" | "quit" => {
                self.add_output_line("Use Esc to exit shell mode".to_string());
                return Ok(());
            }
            "clear" => {
                self.output_history.clear();
                self.add_output_line("Output cleared".to_string());
                return Ok(());
            }
            "help" => {
                self.show_built_in_help();
                return Ok(());
            }
            cmd if cmd.starts_with("cd ") => {
                // each exec is its own process so cd wont persist across commands
                self.add_output_line("Note: 'cd' changes directory only for single command. Use 'pwd' to see current directory.".to_string());
            }
            _ => {}
        }

        if let Some(container) = &self.container {
            match self.execute_in_container(&container.id, &command).await {
                Ok(output) => {
                    for line in output.lines() {
                        self.add_output_line(line.to_string());
                    }
                    if output.trim().is_empty() {
                        self.add_output_line(
                            "(command executed successfully, no output)".to_string(),
                        );
                    }
                }
                Err(e) => {
                    self.add_output_line(format!("❌ Error: {}", e));
                }
            }
        } else {
            self.add_output_line("❌ No container selected".to_string());
        }

        Ok(())
    }

    fn show_built_in_help(&mut self) {
        let help_lines = vec![
            "🐚 Docsee Shell Help:",
            "",
            "Built-in commands:",
            "  help     - Show this help",
            "  clear    - Clear output",
            "  exit     - Use Esc instead",
            "",
            "Shell commands:",
            "  ls       - List files",
            "  pwd      - Print working directory",
            "  cd DIR   - Change directory (single command only)",
            "  cat FILE - Show file contents",
            "  ps       - Show processes",
            "  env      - Show environment variables",
            "",
            "Tips:",
            "  - Use F1 to switch between typing and navigation modes",
            "  - Use Tab to cycle through available shells",
            "  - All commands execute directly in the container",
            "  - Use Ctrl+C to cancel current input",
            "",
        ];

        for line in help_lines {
            self.add_output_line(line.to_string());
        }
    }

    /// runs one command via `docker exec` and returns combined stdout+stderr
    async fn execute_in_container(&self, container_id: &str, command: &str) -> Result<String> {
        let shell = &self.available_shells[self.current_shell];

        let output = tokio::process::Command::new("docker")
            .args(["exec", container_id, shell, "-c", command])
            .output()
            .await
            .context("Failed to execute docker command")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            // stick stderr after stdout with a marker so user knows what came from where
            let mut result = stdout.to_string();
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str("(stderr): ");
                result.push_str(&stderr);
            }

            Ok(result)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!(
                "Command failed (exit code {}): {}",
                output.status.code().unwrap_or(-1),
                stderr
            ))
        }
    }

    /// drops out of the TUI and hands the terminal over to a real `docker exec -it`
    pub async fn start_interactive_shell(&self, container: &Container) -> Result<()> {
        if self.container.is_none() {
            return Err(anyhow::anyhow!("No container selected"));
        }

        // give up raw mode otherwise the inner shell input goes haywire
        disable_raw_mode()?;

        println!(
            "\n🐚 Starting interactive shell in container '{}'...",
            container.name
        );
        println!("💡 This is a full interactive shell session.");
        println!("🎯 Type 'exit' to return to Docsee.\n");

        let shell = &self.available_shells[self.current_shell];
        let mut child = Command::new("docker")
            .args(["exec", "-it", &container.id, shell])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start interactive shell")?;

        let status = child.wait().context("Failed to wait for shell")?;

        if !status.success() {
            println!("\n❌ Shell exited with error code: {:?}", status.code());
        } else {
            println!("\n✅ Shell session ended successfully.");
        }

        println!("Press any key to return to Docsee...");
        io::stdout().flush()?;

        let _ = crossterm::event::read()?;

        // back into raw mode before we redraw the TUI
        enable_raw_mode()?;

        Ok(())
    }

    fn add_to_history(&mut self, command: String) {
        // skip blanks and back-to-back dupes, just like a real shell history
        if command.trim().is_empty() || (self.command_history.back() == Some(&command)) {
            return;
        }

        if self.command_history.len() >= MAX_HISTORY {
            self.command_history.pop_front();
        }

        self.command_history.push_back(command);
    }

    fn add_output_line(&mut self, line: String) {
        // cap at 200 lines so a chatty command doesnt eat all the memory
        if self.output_history.len() >= 200 {
            self.output_history.pop_front();
        }
        self.output_history.push_back(line);

        if self.input_mode == InputMode::Typing {
            self.output_list_state
                .select(Some(self.output_history.len() - 1));
        }
    }

    fn navigate_history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_position = match self.history_position {
            None => Some(self.command_history.len() - 1),
            Some(pos) => {
                if pos > 0 {
                    Some(pos - 1)
                } else {
                    Some(pos)
                }
            }
        };

        if let Some(pos) = new_position {
            if let Some(command) = self.command_history.get(pos) {
                self.current_input = command.clone();
                self.cursor_position = self.current_input.len();
                self.history_position = Some(pos);
            }
        }
    }

    fn navigate_history_down(&mut self) {
        if let Some(pos) = self.history_position {
            if pos < self.command_history.len() - 1 {
                let new_pos = pos + 1;
                if let Some(command) = self.command_history.get(new_pos) {
                    self.current_input = command.clone();
                    self.cursor_position = self.current_input.len();
                    self.history_position = Some(new_pos);
                }
            } else {
                // gone past newest entry, so wipe the line
                self.current_input.clear();
                self.cursor_position = 0;
                self.history_position = None;
            }
        }
    }

    fn scroll_output_up(&mut self) {
        if let Some(selected) = self.output_list_state.selected() {
            if selected > 0 {
                self.output_list_state.select(Some(selected - 1));
            }
        } else if !self.output_history.is_empty() {
            self.output_list_state
                .select(Some(self.output_history.len() - 1));
        }
    }

    fn scroll_output_down(&mut self) {
        if let Some(selected) = self.output_list_state.selected() {
            if selected < self.output_history.len() - 1 {
                self.output_list_state.select(Some(selected + 1));
            }
        }
    }

    fn page_output_up(&mut self) {
        if let Some(selected) = self.output_list_state.selected() {
            let new_selected = selected.saturating_sub(10);
            self.output_list_state.select(Some(new_selected));
        }
    }

    fn page_output_down(&mut self) {
        if let Some(selected) = self.output_list_state.selected() {
            let new_selected = (selected + 10).min(self.output_history.len() - 1);
            self.output_list_state.select(Some(new_selected));
        }
    }

    fn scroll_output_to_top(&mut self) {
        if !self.output_history.is_empty() {
            self.output_list_state.select(Some(0));
        }
    }

    fn scroll_output_to_bottom(&mut self) {
        if !self.output_history.is_empty() {
            self.output_list_state
                .select(Some(self.output_history.len() - 1));
        }
    }

    /// tab through /bin/bash, /bin/sh etc since not every image ships bash
    fn cycle_shell(&mut self) {
        self.current_shell = (self.current_shell + 1) % self.available_shells.len();
        let shell = &self.available_shells[self.current_shell];
        self.status_message = Some(format!("Switched to shell: {}", shell));
        self.add_output_line(format!("🔄 Switched to shell: {}", shell));
    }

    pub fn get_container(&self) -> Option<&Container> {
        self.container.as_ref()
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
