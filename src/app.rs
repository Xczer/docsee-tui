use anyhow::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io::{self, stdout};

use crate::{
    docker::DockerClient,
    events::{AppEvent, EventConfig, EventHandler},
    ui::{cheatsheet::CheatSheet, images::ImagesTab, networks::NetworksTab, volumes::VolumesTab},
};

// Import our enhanced containers tab
use crate::ui::containers::{ContainerViewMode, EnhancedContainersTab};

/// Enhanced application with better navigation and ASCII art
pub struct App {
    /// Docker client for API operations
    #[allow(dead_code)]
    docker_client: DockerClient,
    /// Event handler for terminal input
    event_handler: EventHandler,
    /// Currently selected tab
    current_tab: TabType,
    /// Whether the application should quit
    should_quit: bool,
    /// Whether the cheatsheet is currently shown
    show_cheatsheet: bool,
    /// Whether we're in a container sub-view
    in_container_subview: bool,
    /// Enhanced containers tab with Phase 2 features
    containers_tab: EnhancedContainersTab,
    /// The images tab (existing)
    images_tab: ImagesTab,
    /// The volumes tab (existing)
    volumes_tab: VolumesTab,
    /// The networks tab (existing)
    networks_tab: NetworksTab,
    /// The cheatsheet modal (existing)
    cheatsheet: CheatSheet,
    /// Status message to display globally
    global_status: Option<String>,
}

/// Available tabs in the application
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabType {
    Containers,
    Images,
    Volumes,
    Networks,
}

impl TabType {
    /// Get all available tabs
    pub fn all() -> &'static [TabType] {
        &[
            TabType::Containers,
            TabType::Images,
            TabType::Volumes,
            TabType::Networks,
        ]
    }

    /// Get the display name for the tab
    pub fn name(&self) -> &'static str {
        match self {
            TabType::Containers => "Containers",
            TabType::Images => "Images",
            TabType::Volumes => "Volumes",
            TabType::Networks => "Networks",
        }
    }

    /// Get extra info for the current tab
    pub fn info(&self) -> &'static str {
        match self {
            TabType::Containers => "Docker Container Management",
            TabType::Images => "Docker Image Management",
            TabType::Volumes => "Docker Volume Management",
            TabType::Networks => "Docker Network Management",
        }
    }

    /// Get the next tab (for right arrow navigation)
    pub fn next(&self) -> TabType {
        let tabs = Self::all();
        let current_index = tabs.iter().position(|&tab| tab == *self).unwrap_or(0);
        let next_index = (current_index + 1) % tabs.len();
        tabs[next_index]
    }

    /// Get the previous tab (for left arrow navigation)
    pub fn previous(&self) -> TabType {
        let tabs = Self::all();
        let current_index = tabs.iter().position(|&tab| tab == *self).unwrap_or(0);
        let prev_index = if current_index == 0 {
            tabs.len() - 1
        } else {
            current_index - 1
        };
        tabs[prev_index]
    }
}

impl App {
    /// Create a new enhanced application instance
    pub async fn new(docker_host: &str) -> Result<Self> {
        let docker_client = DockerClient::new(docker_host).await?;
        let event_handler = EventHandler::new(EventConfig::default());

        // Create the enhanced containers tab
        let containers_tab = EnhancedContainersTab::new(docker_client.clone()).await?;

        // Create other tabs (existing implementation)
        let images_tab = ImagesTab::new(docker_client.clone()).await?;
        let volumes_tab = VolumesTab::new(docker_client.clone()).await?;
        let networks_tab = NetworksTab::new(docker_client.clone()).await?;
        let cheatsheet = CheatSheet::new();

        Ok(Self {
            docker_client,
            event_handler,
            current_tab: TabType::Containers,
            should_quit: false,
            show_cheatsheet: false,
            in_container_subview: false,
            containers_tab,
            images_tab,
            volumes_tab,
            networks_tab,
            cheatsheet,
            global_status: None,
        })
    }

    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        // Show welcome message
        self.global_status = Some("🦆 Welcome to Docsee v1.0! Real-time logs, Shell access, Stats monitoring, Advanced search".to_string());

        // Main application loop
        let result = self.main_loop(&mut terminal).await;

        // Cleanup terminal
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        result
    }

    /// Main event loop
    async fn main_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            // Draw the UI
            terminal.draw(|frame| self.draw(frame))?;

            // Handle events
            if let Some(event) = self.event_handler.next().await {
                match event {
                    AppEvent::Key(key) => {
                        self.handle_key_event(key).await?;
                    }
                    AppEvent::Tick => {
                        // Periodic updates (refresh data, etc.)
                        self.handle_tick().await?;
                    }
                    AppEvent::Quit => {
                        self.should_quit = true;
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle key press events with special shell mode handling
    async fn handle_key_event(&mut self, key: crate::events::Key) -> Result<()> {
        use crate::events::Key;

        // Clear any global status after user interaction
        if self.global_status.is_some() {
            self.global_status = None;
        }

        // If cheatsheet is open, only handle escape to close it
        if self.show_cheatsheet {
            if key == Key::Esc {
                self.show_cheatsheet = false;
            }
            return Ok(());
        }

        // Check if we're in a container sub-view (logs, shell, stats)
        self.in_container_subview =
            self.current_tab == TabType::Containers && self.containers_tab.is_in_subview();

        // Special handling for shell mode - intercept raw key events
        if self.in_container_subview
            && self.containers_tab.get_view_mode() == &ContainerViewMode::Shell
        {
            // In shell mode, handle the key directly without converting to actions
            let shell_exit = self.containers_tab.handle_shell_key_raw(key).await?;
            if shell_exit {
                // Shell requested exit, this will update the view mode
                return Ok(());
            }
            // Don't process any other global keys in shell mode
            return Ok(());
        }

        // Handle global keys first (unless in non-shell sub-view)
        if !self.in_container_subview {
            match key {
                Key::Quit => {
                    self.should_quit = true;
                    return Ok(());
                }
                Key::Cheatsheet => {
                    self.show_cheatsheet = true;
                    return Ok(());
                }
                Key::Left => {
                    self.current_tab = self.current_tab.previous();
                    return Ok(());
                }
                Key::Right => {
                    self.current_tab = self.current_tab.next();
                    return Ok(());
                }
                _ => {}
            }
        } else {
            // In non-shell sub-view, allow some global keys
            match key {
                Key::Quit => {
                    // Force exit sub-view first, then quit
                    self.containers_tab.force_exit_subview().await?;
                    self.should_quit = true;
                    return Ok(());
                }
                Key::Cheatsheet => {
                    self.show_cheatsheet = true;
                    return Ok(());
                }
                // Tab navigation is disabled in sub-views for safety
                _ => {}
            }
        }

        // Pass key to current tab
        match self.current_tab {
            TabType::Containers => {
                self.containers_tab.handle_key(key).await?;
            }
            TabType::Images => {
                self.images_tab.handle_key(key).await?;
            }
            TabType::Volumes => {
                self.volumes_tab.handle_key(key).await?;
            }
            TabType::Networks => {
                self.networks_tab.handle_key(key).await?;
            }
        }

        Ok(())
    }

    /// Handle periodic tick events
    async fn handle_tick(&mut self) -> Result<()> {
        // Refresh current tab data
        match self.current_tab {
            TabType::Containers => {
                self.containers_tab.refresh().await?;
            }
            TabType::Images => {
                self.images_tab.refresh().await?;
            }
            TabType::Volumes => {
                self.volumes_tab.refresh().await?;
            }
            TabType::Networks => {
                self.networks_tab.refresh().await?;
            }
        }

        Ok(())
    }

    /// Draw the user interface
    fn draw(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Create main layout: ASCII title, navigation header, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // ASCII Art Title
                Constraint::Length(3), // Navigation header with prev/current/next
                Constraint::Min(0),    // Content area
                Constraint::Length(1), // Footer
            ])
            .split(size);

        // Draw ASCII art title
        self.draw_ascii_title(frame, chunks[0]);

        // Draw navigation header
        self.draw_navigation_header(frame, chunks[1]);

        // Draw content based on current tab
        if self.show_cheatsheet {
            self.cheatsheet.draw(frame, chunks[2]);
        } else {
            match self.current_tab {
                TabType::Containers => {
                    self.containers_tab.draw(frame, chunks[2]);
                }
                TabType::Images => {
                    self.images_tab.draw(frame, chunks[2]);
                }
                TabType::Volumes => {
                    self.volumes_tab.draw(frame, chunks[2]);
                }
                TabType::Networks => {
                    self.networks_tab.draw(frame, chunks[2]);
                }
            }
        }

        // Draw footer
        self.draw_footer(frame, chunks[3]);
    }

    /// Draw ASCII art title
    fn draw_ascii_title(&mut self, frame: &mut Frame, area: Rect) {
        let ascii_art = vec![
            Line::from(Span::styled(
                "██████╗  ██████╗  ██████╗███████╗███████╗███████╗",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "██╔══██╗██╔═══██╗██╔════╝██╔════╝██╔════╝██╔════╝",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "██║  ██║██║   ██║██║     ███████╗█████╗  █████╗  ",
                Style::default().fg(Color::Blue),
            )),
            Line::from(Span::styled(
                "██║  ██║██║   ██║██║     ╚════██║██╔══╝  ██╔══╝  ",
                Style::default().fg(Color::Blue),
            )),
            Line::from(Span::styled(
                "██████╔╝╚██████╔╝╚██████╗███████║███████╗███████╗",
                Style::default().fg(Color::Magenta),
            )),
            Line::from(Span::styled(
                "╚═════╝  ╚═════╝  ╚═════╝╚══════╝╚══════╝╚══════╝",
                Style::default().fg(Color::Magenta),
            )),
            Line::from(Span::styled(
                "            🦆 Docker Management TUI v1.0",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            )),
        ];

        let title_paragraph = Paragraph::new(ascii_art)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        frame.render_widget(title_paragraph, area);
    }

    /// Draw the navigation header with prev/current/next tabs
    fn draw_navigation_header(&mut self, frame: &mut Frame, area: Rect) {
        // Don't show navigation in sub-views to avoid confusion
        if self.in_container_subview {
            let sub_view_info_line = Line::from(vec![
                Span::styled("📍 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    self.containers_tab.get_view_mode().name(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - Press "),
                Span::styled(
                    "Esc",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to return to main view"),
            ]);

            let sub_view_info = Paragraph::new(sub_view_info_line)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));

            frame.render_widget(sub_view_info, area);
            return;
        }

        // Create horizontal layout for navigation
        let nav_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Previous tab
                Constraint::Percentage(50), // Current tab (center)
                Constraint::Percentage(25), // Next tab
            ])
            .split(area);

        let prev_tab = self.current_tab.previous();
        let next_tab = self.current_tab.next();

        // Draw previous tab (left)
        let prev_text = Line::from(vec![
            Span::styled("← ", Style::default().fg(Color::Gray)),
            Span::styled(prev_tab.name(), Style::default().fg(Color::Gray)),
        ]);
        let prev_paragraph = Paragraph::new(prev_text)
            .alignment(Alignment::Left)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(prev_paragraph, nav_chunks[0]);

        // Draw current tab (center)
        let current_text = vec![
            Line::from(vec![Span::styled(
                self.current_tab.name(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                self.current_tab.info(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::ITALIC),
            )]),
        ];
        let current_paragraph = Paragraph::new(current_text)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
        frame.render_widget(current_paragraph, nav_chunks[1]);

        // Draw next tab (right)
        let next_text = Line::from(vec![
            Span::styled(next_tab.name(), Style::default().fg(Color::Gray)),
            Span::styled(" →", Style::default().fg(Color::Gray)),
        ]);
        let next_paragraph = Paragraph::new(next_text)
            .alignment(Alignment::Right)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(next_paragraph, nav_chunks[2]);
    }

    /// Draw the footer with help information
    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = if self.in_container_subview {
            // Show sub-view specific help
            match self.containers_tab.get_view_mode() {
                ContainerViewMode::Logs => "Logs: ↑/↓ scroll | f follow | t timestamps | c clear | Esc back",
                ContainerViewMode::Shell => "Shell: F1 toggle mode | Type commands | ↑/↓ history | Tab switch shell | Esc back",
                ContainerViewMode::Stats => "Stats: ←/→ switch view | r reset | p pause | +/- interval | Esc back",
                _ => "Container sub-view - Press Esc to return",
            }
        } else if self.show_cheatsheet {
            "Cheatsheet: Press Esc to close"
        } else {
            "Global: q quit | c help | ←/→ navigate | Container: l logs | e shell | s stats | / search"
        };

        let status_text = if let Some(ref global_status) = self.global_status {
            format!(" | {global_status}")
        } else {
            match self.current_tab {
                TabType::Containers => {
                    if let Some(status) = self.containers_tab.get_status() {
                        format!(" | {status}")
                    } else {
                        String::new()
                    }
                }
                _ => String::new(),
            }
        };

        let footer = Paragraph::new(format!("{footer_text}{status_text}"))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(footer, area);
    }
}
