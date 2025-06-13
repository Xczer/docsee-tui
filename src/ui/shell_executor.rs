use anyhow::{Context, Result};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
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

/// Maximum number of command history entries
const MAX_HISTORY: usize = 100;

/// Shell execution session
pub struct ShellExecutor {
    /// Docker client for operations
    _docker_client: DockerClient,
    /// Container being accessed
    container: Option<Container>,
    /// Command history
    command_history: VecDeque<String>,
    /// Current input buffer
    current_input: String,
    /// History navigation position
    history_position: Option<usize>,
    /// Status message
    status_message: Option<String>,
    /// Available shells to try
    available_shells: Vec<String>,
    /// Currently selected shell
    current_shell: usize,
}

impl ShellExecutor {
    /// Create a new shell executor
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            _docker_client: docker_client,
            container: None,
            command_history: VecDeque::with_capacity(MAX_HISTORY),
            current_input: String::new(),
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
        }
    }

    /// Set the container for shell access
    pub fn set_container(&mut self, container: Container) {
        self.container = Some(container);
        self.current_input.clear();
        self.history_position = None;
        self.status_message = Some(format!("Ready to execute commands in '{}'", self.container.as_ref().unwrap().name));
    }

    /// Handle key events
    pub async fn handle_key(&mut self, key: Key) -> Result<bool> {
        match key {
            Key::Enter => {
                self.execute_command().await?;
                Ok(false) // Stay in shell mode
            }
            Key::Up => {
                self.navigate_history_up();
                Ok(false)
            }
            Key::Down => {
                self.navigate_history_down();
                Ok(false)
            }
            Key::Char(c) => {
                self.current_input.push(c);
                self.history_position = None;
                Ok(false)
            }
            Key::Backspace => {
                self.current_input.pop();
                self.history_position = None;
                Ok(false)
            }
            Key::Ctrl('c') => {
                self.current_input.clear();
                self.history_position = None;
                Ok(false)
            }
            Key::Ctrl('l') => {
                // Clear screen equivalent
                self.status_message = Some("Screen cleared".to_string());
                Ok(false)
            }
            Key::Tab => {
                self.cycle_shell();
                Ok(false)
            }
            Key::Esc => {
                Ok(true) // Exit shell mode
            }
            _ => Ok(false),
        }
    }

    /// Draw the shell interface
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Split area into header, history, and input
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header
                Constraint::Min(0),    // Command history
                Constraint::Length(3), // Input line
            ])
            .split(area);

        // Draw header
        self.draw_header(frame, chunks[0]);

        // Draw command history
        self.draw_history(frame, chunks[1]);

        // Draw input line
        self.draw_input(frame, chunks[2]);
    }

    /// Draw the header with container info
    fn draw_header(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("No Container");

        let current_shell = &self.available_shells[self.current_shell];

        let title = format!("Shell: {} ({})", container_name, current_shell);

        let title_text = if let Some(ref message) = self.status_message {
            format!("{} - {}", title, message)
        } else {
            title
        };

        let header = Paragraph::new(vec![
            Line::from(vec![Span::raw(title_text)]),
            Line::from(vec![
                Span::styled("Controls: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" Execute | "),
                Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                Span::raw(" History | "),
                Span::styled("Tab", Style::default().fg(Color::Yellow)),
                Span::raw(" Switch Shell | "),
                Span::styled("Ctrl+C", Style::default().fg(Color::Yellow)),
                Span::raw(" Clear | "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" Exit"),
            ]),
            Line::from(vec![
                Span::styled("Warning: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("Commands will be executed directly in the container!"),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(header, area);
    }

    /// Draw command history
    fn draw_history(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .command_history
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{}> ", i + 1), Style::default().fg(Color::DarkGray)),
                    Span::raw(cmd.clone()),
                ]))
            })
            .collect();

        let history_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Command History"))
            .highlight_style(Style::default().bg(Color::DarkGray));

        let mut list_state = ListState::default();
        if !self.command_history.is_empty() {
            list_state.select(Some(self.command_history.len() - 1));
        }

        frame.render_stateful_widget(history_list, area, &mut list_state);
    }

    /// Draw input line
    fn draw_input(&mut self, frame: &mut Frame, area: Rect) {
        let container_name = self
            .container
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("container");

        let prompt = format!("{}@{}:~$ ", "user", container_name);
        
        let input_line = Paragraph::new(Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&self.current_input),
            Span::styled("█", Style::default().fg(Color::White)), // Cursor
        ]))
        .block(Block::default().borders(Borders::ALL).title("Command Input"));

        frame.render_widget(input_line, area);
    }

    /// Execute the current command
    async fn execute_command(&mut self) -> Result<()> {
        if self.current_input.trim().is_empty() {
            return Ok(());
        }

        let command = self.current_input.clone();
        self.add_to_history(command.clone());
        self.current_input.clear();
        self.history_position = None;

        // Handle built-in commands
        match command.trim() {
            "exit" | "quit" => {
                self.status_message = Some("Use Esc to exit shell mode".to_string());
                return Ok(());
            }
            "clear" => {
                self.status_message = Some("Screen cleared".to_string());
                return Ok(());
            }
            "help" => {
                self.status_message = Some("Available: exit, clear, help, or any shell command".to_string());
                return Ok(());
            }
            _ => {}
        }

        // Execute command in container
        if let Some(container) = &self.container {
            match self.execute_in_container(&container.id, &command).await {
                Ok(output) => {
                    self.status_message = Some(format!("Command executed: {}", output.trim()));
                }
                Err(e) => {
                    self.status_message = Some(format!("Error: {}", e));
                }
            }
        } else {
            self.status_message = Some("No container selected".to_string());
        }

        Ok(())
    }

    /// Execute command in the Docker container
    async fn execute_in_container(&self, container_id: &str, command: &str) -> Result<String> {
        // For now, we'll use docker exec via system command
        // In a production app, you might want to use the Docker API directly
        let shell = &self.available_shells[self.current_shell];
        
        let output = tokio::process::Command::new("docker")
            .args(&["exec", container_id, shell, "-c", command])
            .output()
            .await
            .context("Failed to execute docker command")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Command failed: {}", stderr))
        }
    }

    /// Start an interactive shell session
    pub async fn start_interactive_shell(&self, container: &Container) -> Result<()> {
        if self.container.is_none() {
            return Err(anyhow::anyhow!("No container selected"));
        }

        // Disable TUI raw mode temporarily
        disable_raw_mode()?;

        // Clear the status message
        println!("\n🐚 Starting interactive shell in container '{}'...", container.name);
        println!("Type 'exit' to return to Docsee.\n");

        // Start interactive docker exec
        let shell = &self.available_shells[self.current_shell];
        let mut child = Command::new("docker")
            .args(&["exec", "-it", &container.id, shell])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start interactive shell")?;

        // Wait for the shell to exit
        let status = child.wait().context("Failed to wait for shell")?;

        if !status.success() {
            println!("\n❌ Shell exited with error code: {:?}", status.code());
        } else {
            println!("\n✅ Shell session ended.");
        }

        println!("Press any key to return to Docsee...");
        io::stdout().flush()?;

        // Wait for any key press
        let _ = crossterm::event::read()?;

        // Re-enable raw mode
        enable_raw_mode()?;

        Ok(())
    }

    /// Add command to history
    fn add_to_history(&mut self, command: String) {
        // Don't add empty commands or duplicates
        if command.trim().is_empty() || 
           self.command_history.back().map_or(false, |last| last == &command) {
            return;
        }

        // Remove oldest if at capacity
        if self.command_history.len() >= MAX_HISTORY {
            self.command_history.pop_front();
        }

        self.command_history.push_back(command);
    }

    /// Navigate up in command history
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
                self.history_position = Some(pos);
            }
        }
    }

    /// Navigate down in command history
    fn navigate_history_down(&mut self) {
        if let Some(pos) = self.history_position {
            if pos < self.command_history.len() - 1 {
                let new_pos = pos + 1;
                if let Some(command) = self.command_history.get(new_pos) {
                    self.current_input = command.clone();
                    self.history_position = Some(new_pos);
                }
            } else {
                // At the end of history, clear input
                self.current_input.clear();
                self.history_position = None;
            }
        }
    }

    /// Cycle through available shells
    fn cycle_shell(&mut self) {
        self.current_shell = (self.current_shell + 1) % self.available_shells.len();
        let shell = &self.available_shells[self.current_shell];
        self.status_message = Some(format!("Switched to shell: {}", shell));
    }

    /// Get the current container
    pub fn get_container(&self) -> Option<&Container> {
        self.container.as_ref()
    }

    /// Clear the status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
