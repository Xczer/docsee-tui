use anyhow::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Tabs},
    Frame, Terminal,
};
use std::io::{self, stdout};

use crate::{
    docker::DockerClient,
    events::{AppEvent, EventConfig, EventHandler},
    ui::{containers::ContainersTab, images::ImagesTab, networks::NetworksTab, volumes::VolumesTab, cheatsheet::CheatSheet},
};

/// The main application state
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
    /// The containers tab
    containers_tab: ContainersTab,
    /// The images tab
    images_tab: ImagesTab,
    /// The volumes tab
    volumes_tab: VolumesTab,
    /// The networks tab
    networks_tab: NetworksTab,
    /// The cheatsheet modal
    cheatsheet: CheatSheet,
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
    /// Create a new application instance
    pub async fn new(docker_host: &str) -> Result<Self> {
        let docker_client = DockerClient::new(docker_host).await?;
        let event_handler = EventHandler::new(EventConfig::default());
        let containers_tab = ContainersTab::new(docker_client.clone()).await?;
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
            containers_tab,
            images_tab,
            volumes_tab,
            networks_tab,
            cheatsheet,
        })
    }

    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

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

        // If cheatsheet is open, only handle escape to close it
        if self.show_cheatsheet {
            if key == Key::Esc {
                self.show_cheatsheet = false;
            }
            return Ok(());
        }

        // Global key handlers
        match key {
            Key::Quit => {
                self.should_quit = true;
            }
            Key::Cheatsheet => {
                self.show_cheatsheet = true;
            }
            Key::Left => {
                self.current_tab = self.current_tab.previous();
            }
            Key::Right => {
                self.current_tab = self.current_tab.next();
            }
            _ => {
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

        // Create main layout: tabs at top, content below
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content area
            ])
            .split(size);

        // Draw tab bar
        self.draw_tabs(frame, chunks[0]);

        // Draw content based on current tab
        if self.show_cheatsheet {
            self.cheatsheet.draw(frame, chunks[1]);
        } else {
            match self.current_tab {
                TabType::Containers => {
                    self.containers_tab.draw(frame, chunks[1]);
                }
                TabType::Images => {
                    self.images_tab.draw(frame, chunks[1]);
                }
                TabType::Volumes => {
                    self.volumes_tab.draw(frame, chunks[1]);
                }
                TabType::Networks => {
                    self.networks_tab.draw(frame, chunks[1]);
                }
            }
        }
    }

    /// Draw the tab bar at the top
    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        let tab_names: Vec<&str> = TabType::all().iter().map(|tab| tab.name()).collect();
        let current_index = TabType::all()
            .iter()
            .position(|&tab| tab == self.current_tab)
            .unwrap_or(0);

        let tabs = Tabs::new(tab_names)
            .block(Block::default().borders(Borders::ALL).title("Docsee - Docker Manager"))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            )
            .select(current_index);

        frame.render_widget(tabs, area);
    }
}

/*
EXPLANATION:
- App is the main application struct that coordinates everything
- TabType enum defines our four tabs with navigation methods
- new() creates the app, initializes Docker client and UI components
- run() sets up the terminal and starts the main loop
- main_loop() is the core event loop that draws UI and handles events
- handle_key_event() processes keyboard input:
  - Global keys (q, c, arrows) are handled first
  - Tab-specific keys are passed to the current tab
- handle_tick() refreshes data periodically
- draw() renders the entire UI using Ratatui
- draw_tabs() creates the tab bar at the top
- Terminal setup/cleanup ensures proper restoration when the app exits
- All four tabs (Containers, Images, Volumes, Networks) are now fully implemented

FIXES APPLIED:
- Removed unused imports: Line, Span, Paragraph
- Kept only the necessary imports for the current functionality
*/
