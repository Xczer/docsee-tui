use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io::{self, stdout};

use crate::{
    config::AppConfig,
    docker::DockerClient,
    events::{AppEvent, EventConfig, EventHandler},
    theme::Theme,
    ui::{
        cheatsheet::CheatSheet, images::ImagesTab, networks::NetworksTab, system::SystemTab,
        volumes::VolumesTab,
    },
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
    /// The system dashboard tab
    system_tab: SystemTab,
    /// The cheatsheet modal (existing)
    cheatsheet: CheatSheet,
    /// Status message to display globally
    global_status: Option<String>,
    /// Color theme
    theme: Theme,
    /// Whether mouse support is enabled
    mouse_enabled: bool,
    /// Stored layout rects for mouse hit-testing
    nav_area: Option<Rect>,
    content_area: Option<Rect>,
}

/// Available tabs in the application
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabType {
    Containers,
    Images,
    Volumes,
    Networks,
    System,
}

impl TabType {
    /// Get all available tabs
    pub fn all() -> &'static [TabType] {
        &[
            TabType::Containers,
            TabType::Images,
            TabType::Volumes,
            TabType::Networks,
            TabType::System,
        ]
    }

    /// Get the display name for the tab
    pub fn name(&self) -> &'static str {
        match self {
            TabType::Containers => "Containers",
            TabType::Images => "Images",
            TabType::Volumes => "Volumes",
            TabType::Networks => "Networks",
            TabType::System => "System",
        }
    }

    /// Get extra info for the current tab
    pub fn info(&self) -> &'static str {
        match self {
            TabType::Containers => "Docker Container Management",
            TabType::Images => "Docker Image Management",
            TabType::Volumes => "Docker Volume Management",
            TabType::Networks => "Docker Network Management",
            TabType::System => "Docker System Dashboard",
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
    pub async fn new(config: &AppConfig, theme: Theme) -> Result<Self> {
        let docker_client = DockerClient::new(&config.general.docker_host).await?;
        let event_config = EventConfig {
            tick_rate: std::time::Duration::from_millis(config.general.tick_rate_ms),
            ..EventConfig::default()
        };
        let event_handler = EventHandler::new(event_config);

        let default_tab = match config.general.default_tab.to_lowercase().as_str() {
            "images" => TabType::Images,
            "volumes" => TabType::Volumes,
            "networks" => TabType::Networks,
            "system" => TabType::System,
            _ => TabType::Containers,
        };

        // Create all tabs
        let containers_tab = EnhancedContainersTab::new(docker_client.clone(), theme.clone()).await?;
        let images_tab = ImagesTab::new(docker_client.clone(), theme.clone()).await?;
        let volumes_tab = VolumesTab::new(docker_client.clone(), theme.clone()).await?;
        let networks_tab = NetworksTab::new(docker_client.clone(), theme.clone()).await?;
        let system_tab = SystemTab::new(docker_client.clone(), theme.clone()).await?;
        let cheatsheet = CheatSheet::new();
        let mouse_enabled = config.mouse.enabled;

        Ok(Self {
            docker_client,
            event_handler,
            current_tab: default_tab,
            should_quit: false,
            show_cheatsheet: false,
            in_container_subview: false,
            containers_tab,
            images_tab,
            volumes_tab,
            networks_tab,
            system_tab,
            cheatsheet,
            global_status: None,
            theme,
            mouse_enabled,
            nav_area: None,
            content_area: None,
        })
    }

    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        if self.mouse_enabled {
            stdout().execute(EnableMouseCapture)?;
        }
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        // Show welcome message
        self.global_status = Some("Welcome to Docsee v2.0! Inspect, Topology, Compose groups, System dashboard".to_string());

        // Main application loop
        let result = self.main_loop(&mut terminal).await;

        // Cleanup terminal
        if self.mouse_enabled {
            stdout().execute(DisableMouseCapture)?;
        }
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
                    AppEvent::Mouse(mouse_event) => {
                        self.handle_mouse_event(mouse_event);
                    }
                    AppEvent::Tick => {
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

        // Check if we're in a container sub-view (logs, shell, stats, inspect, topology)
        self.in_container_subview =
            self.current_tab == TabType::Containers && self.containers_tab.is_in_subview();

        // Special handling for shell mode - intercept raw key events
        if self.in_container_subview
            && self.containers_tab.get_view_mode() == &ContainerViewMode::Shell
        {
            let shell_exit = self.containers_tab.handle_shell_key_raw(key).await?;
            if shell_exit {
                return Ok(());
            }
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
            match key {
                Key::Quit => {
                    self.containers_tab.force_exit_subview().await?;
                    self.should_quit = true;
                    return Ok(());
                }
                Key::Cheatsheet => {
                    self.show_cheatsheet = true;
                    return Ok(());
                }
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
            TabType::System => {
                self.system_tab.handle_key(key).await?;
            }
        }

        Ok(())
    }

    /// Handle periodic tick events - catches Docker disconnects gracefully
    async fn handle_tick(&mut self) -> Result<()> {
        let result = match self.current_tab {
            TabType::Containers => self.containers_tab.refresh().await,
            TabType::Images => self.images_tab.refresh().await,
            TabType::Volumes => self.volumes_tab.refresh().await,
            TabType::Networks => self.networks_tab.refresh().await,
            TabType::System => self.system_tab.refresh().await,
        };

        if let Err(e) = result {
            let msg = format!("{}", e);
            if msg.contains("connection") || msg.contains("refused") || msg.contains("daemon") || msg.contains("socket") {
                self.global_status = Some("Docker disconnected - reconnect Docker and press any key".to_string());
            }
            // Don't propagate - just show the error, keep the app alive
        }

        Ok(())
    }

    /// Handle mouse events
    fn handle_mouse_event(&mut self, mouse_event: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseEventKind, MouseButton};

        let col = mouse_event.column;
        let row = mouse_event.row;

        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check nav area click for tab switching
                if let Some(nav) = self.nav_area {
                    if row >= nav.y && row < nav.y + nav.height && col >= nav.x && col < nav.x + nav.width {
                        // Divide into 3 equal sections for prev/current/next
                        let third = nav.width / 3;
                        if col < nav.x + third {
                            self.current_tab = self.current_tab.previous();
                        } else if col >= nav.x + third * 2 {
                            self.current_tab = self.current_tab.next();
                        }
                        return;
                    }
                }

                // Check content area click for row selection
                if let Some(content) = self.content_area {
                    if row >= content.y && row < content.y + content.height {
                        let clicked_row = (row - content.y) as usize;
                        // Subtract header rows (border + header + border = ~3 rows)
                        if clicked_row >= 3 {
                            let item_index = clicked_row - 3;
                            match self.current_tab {
                                TabType::Containers => {
                                    self.containers_tab.select_row(item_index);
                                }
                                TabType::Images => {
                                    self.images_tab.select_row(item_index);
                                }
                                TabType::Volumes => {
                                    self.volumes_tab.select_row(item_index);
                                }
                                TabType::Networks => {
                                    self.networks_tab.select_row(item_index);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                match self.current_tab {
                    TabType::Containers => self.containers_tab.scroll_up(),
                    TabType::Images => self.images_tab.scroll_up(),
                    TabType::Volumes => self.volumes_tab.scroll_up(),
                    TabType::Networks => self.networks_tab.scroll_up(),
                    _ => {}
                }
            }
            MouseEventKind::ScrollDown => {
                match self.current_tab {
                    TabType::Containers => self.containers_tab.scroll_down(),
                    TabType::Images => self.images_tab.scroll_down(),
                    TabType::Volumes => self.volumes_tab.scroll_down(),
                    TabType::Networks => self.networks_tab.scroll_down(),
                    _ => {}
                }
            }
            _ => {}
        }
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

        // Store layout rects for mouse hit-testing
        self.nav_area = Some(chunks[1]);
        self.content_area = Some(chunks[2]);

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
                TabType::System => {
                    self.system_tab.draw(frame, chunks[2]);
                }
            }
        }

        // Draw footer
        self.draw_footer(frame, chunks[3]);
    }

    /// Draw ASCII art title
    fn draw_ascii_title(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let ascii_art = vec![
            Line::from(Span::styled(
                "██████╗  ██████╗  ██████╗███████╗███████╗███████╗",
                Style::default()
                    .fg(t.title_1)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "██╔══██╗██╔═══██╗██╔════╝██╔════╝██╔════╝██╔════╝",
                Style::default().fg(t.title_1),
            )),
            Line::from(Span::styled(
                "██║  ██║██║   ██║██║     ███████╗█████╗  █████╗  ",
                Style::default().fg(t.title_2),
            )),
            Line::from(Span::styled(
                "██║  ██║██║   ██║██║     ╚════██║██╔══╝  ██╔══╝  ",
                Style::default().fg(t.title_2),
            )),
            Line::from(Span::styled(
                "██████╔╝╚██████╔╝╚██████╗███████║███████╗███████╗",
                Style::default().fg(t.title_3),
            )),
            Line::from(Span::styled(
                "╚═════╝  ╚═════╝  ╚═════╝╚══════╝╚══════╝╚══════╝",
                Style::default().fg(t.title_3),
            )),
            Line::from(Span::styled(
                "            Docker Management TUI v2.0",
                Style::default()
                    .fg(t.accent)
                    .add_modifier(Modifier::ITALIC),
            )),
        ];

        let title_paragraph = Paragraph::new(ascii_art)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(t.border)),
            );

        frame.render_widget(title_paragraph, area);
    }

    /// Draw the navigation header with prev/current/next tabs
    fn draw_navigation_header(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;

        // Don't show navigation in sub-views to avoid confusion
        if self.in_container_subview {
            let sub_view_info_line = Line::from(vec![
                Span::styled(">> ", Style::default().fg(t.accent)),
                Span::styled(
                    self.containers_tab.get_view_mode().name(),
                    Style::default()
                        .fg(t.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - Press "),
                Span::styled(
                    "Esc",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
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
            Span::styled("<- ", Style::default().fg(t.muted)),
            Span::styled(prev_tab.name(), Style::default().fg(t.muted)),
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
                    .fg(t.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                self.current_tab.info(),
                Style::default()
                    .fg(t.fg)
                    .add_modifier(Modifier::ITALIC),
            )]),
        ];
        let current_paragraph = Paragraph::new(current_text)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(t.border_focused)),
            );
        frame.render_widget(current_paragraph, nav_chunks[1]);

        // Draw next tab (right)
        let next_text = Line::from(vec![
            Span::styled(next_tab.name(), Style::default().fg(t.muted)),
            Span::styled(" ->", Style::default().fg(t.muted)),
        ]);
        let next_paragraph = Paragraph::new(next_text)
            .alignment(Alignment::Right)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(next_paragraph, nav_chunks[2]);
    }

    /// Draw the footer with help information
    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = if self.in_container_subview {
            match self.containers_tab.get_view_mode() {
                ContainerViewMode::Logs => "Logs: Up/Down scroll | f follow | t timestamps | x export txt | X export json | Esc back",
                ContainerViewMode::Shell => "Shell: F1 toggle mode | Type commands | Up/Down history | Tab switch shell | Esc back",
                ContainerViewMode::Stats => "Stats: Left/Right switch view | r reset | p pause | +/- interval | Esc back",
                ContainerViewMode::Inspect => "Inspect: Left/Right sections | Up/Down scroll | Esc back",
                ContainerViewMode::Topology => "Topology: Up/Down scroll | r refresh | Esc back",
                _ => "Container sub-view - Press Esc to return",
            }
        } else if self.show_cheatsheet {
            "Cheatsheet: Press Esc to close"
        } else {
            match self.current_tab {
                TabType::Containers => "q quit | c help | o sort | Space select | l logs | e shell | s stats | C compose up | W compose down",
                TabType::Images => "q quit | c help | o sort | D delete | p prune | P pull | R run container",
                TabType::System => "q quit | c help | Left/Right views | r refresh",
                _ => "q quit | c help | o sort | D delete | p prune",
            }
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
            .style(Style::default().fg(self.theme.muted))
            .alignment(Alignment::Center);

        frame.render_widget(footer, area);
    }
}
