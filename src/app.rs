use anyhow::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame, Terminal,
};
use std::io::{self, stdout};

use crate::{
    docker::DockerClient,
    events::{AppEvent, EventConfig, EventHandler},
    ui::{
        cheatsheet::CheatSheet,
        images::ImagesTab,
        networks::NetworksTab,
        volumes::VolumesTab,
    },
};

// Import our enhanced containers tab
use crate::ui::containers::{EnhancedContainersTab, ContainerViewMode};

/// Enhanced application with Phase 2 features
pub struct App {
    /// Docker client for API operations
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
        self.global_status = Some("🦆 Welcome to Docsee! New features: Real-time logs, Shell access, Stats monitoring, Advanced search".to_string());

        // Main application loop
        let result = self.main_loop(&mut terminal).await;

        // Cleanup terminal
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        result
    }

    /// Main event loop
    async fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
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

    /// Handle key press events
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
        self.in_container_subview = self.current_tab == TabType::Containers &&
                                    self.containers_tab.is_in_subview();

        // Handle global keys first (unless in sub-view)
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
            // In sub-view, allow some global keys
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

        // Create main layout: header, tabs, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header with title and status
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content area
                Constraint::Length(1), // Footer
            ])
            .split(size);

        // Draw header
        self.draw_header(frame, chunks[0]);

        // Draw tab bar
        self.draw_tabs(frame, chunks[1]);

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

    /// Draw the header with title and status
    fn draw_header(&mut self, frame: &mut Frame, area: Rect) {
        let status_text = if let Some(ref global_status) = self.global_status {
            global_status.clone()
        } else {
            match self.current_tab {
                TabType::Containers => {
                    self.containers_tab.get_status().unwrap_or_else(|| "Ready".to_string())
                }
                _ => "Ready".to_string(),
            }
        };

        let header_text = if self.in_container_subview {
            // Show current sub-view in header
            format!("🦆 Docsee - {} | {}",
                   self.containers_tab.get_view_mode().name(),
                   status_text)
        } else {
            format!("🦆 Docsee - Docker Management | {}", status_text)
        };

        let header = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(header, area);
    }

    /// Draw the tab bar at the top
    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        // Don't show tabs in sub-views to avoid confusion
        if self.in_container_subview {
            let sub_view_info = Paragraph::new(
                "📍 Sub-view active - Press Esc to return to main view, or use global shortcuts"
            )
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL));

            frame.render_widget(sub_view_info, area);
            return;
        }

        let tab_names: Vec<&str> = TabType::all().iter().map(|tab| tab.name()).collect();
        let current_index = TabType::all()
            .iter()
            .position(|&tab| tab == self.current_tab)
            .unwrap_or(0);

        let tabs = Tabs::new(tab_names)
            .block(Block::default().borders(Borders::ALL).title("Navigation"))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            )
            .select(current_index);

        frame.render_widget(tabs, area);
    }

    /// Draw the footer with help information
    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = if self.in_container_subview {
            // Show sub-view specific help
            match self.containers_tab.get_view_mode() {
                ContainerViewMode::Logs => "Logs: ↑/↓ scroll | f follow | t timestamps | c clear | Esc back",
                ContainerViewMode::Shell => "Shell: Type commands | ↑/↓ history | Tab switch shell | Esc back",
                ContainerViewMode::Stats => "Stats: ←/→ switch view | r reset | p pause | +/- interval | Esc back",
                _ => "Container sub-view - Press Esc to return",
            }
        } else if self.show_cheatsheet {
            "Cheatsheet: Press Esc to close"
        } else {
            "Global: q quit | c help | ←/→ tabs | Container: l logs | e shell | s stats | / search"
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(footer, area);
    }
}
