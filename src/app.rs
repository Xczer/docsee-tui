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
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame, Terminal,
};
use std::io::{self, stdout};

use crate::{
    config::AppConfig,
    docker::DockerClient,
    events::{AppEvent, EventConfig, EventHandler},
    theme::{Theme, SPINNER_FRAMES},
    ui::{
        cheatsheet::CheatSheet, images::ImagesTab, networks::NetworksTab, system::SystemTab,
        volumes::VolumesTab,
    },
};

use crate::ui::containers::{ContainerViewMode, EnhancedContainersTab};

// small auto-dismissing notification shown in the corner
#[derive(Debug, Clone)]
struct Toast {
    message: String,
    created_at: std::time::Instant,
    severity: ToastSeverity,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum ToastSeverity {
    Success,
    Info,
    Warning,
    Error,
}

impl Toast {
    fn new(message: String, severity: ToastSeverity) -> Self {
        Self {
            message,
            created_at: std::time::Instant::now(),
            severity,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > std::time::Duration::from_secs(4)
    }

    fn color(&self, theme: &Theme) -> ratatui::style::Color {
        match self.severity {
            ToastSeverity::Success => theme.success,
            ToastSeverity::Info => theme.info,
            ToastSeverity::Warning => theme.warning,
            ToastSeverity::Error => theme.error,
        }
    }
}

/// top level app state - every tab plus shared stuff (theme, toasts, conn status)
pub struct App {
    #[allow(dead_code)]
    docker_client: DockerClient,
    event_handler: EventHandler,
    current_tab: TabType,
    should_quit: bool,
    show_cheatsheet: bool,
    in_container_subview: bool,
    containers_tab: EnhancedContainersTab,
    images_tab: ImagesTab,
    volumes_tab: VolumesTab,
    networks_tab: NetworksTab,
    system_tab: SystemTab,
    cheatsheet: CheatSheet,
    theme: Theme,
    mouse_enabled: bool,
    // we stash the rects each frame so mouse clicks can be mapped back to a region
    nav_area: Option<Rect>,
    content_area: Option<Rect>,
    toasts: Vec<Toast>,
    spinner_frame: usize,
    is_loading: bool,
    docker_connected: bool,
    docker_version: String, // cached so we dont hit the daemon every render
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabType {
    Containers,
    Images,
    Volumes,
    Networks,
    System,
}

impl TabType {
    pub fn all() -> &'static [TabType] {
        &[
            TabType::Containers,
            TabType::Images,
            TabType::Volumes,
            TabType::Networks,
            TabType::System,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            TabType::Containers => "Containers",
            TabType::Images => "Images",
            TabType::Volumes => "Volumes",
            TabType::Networks => "Networks",
            TabType::System => "System",
        }
    }

    pub fn info(&self) -> &'static str {
        match self {
            TabType::Containers => "Docker Container Management",
            TabType::Images => "Docker Image Management",
            TabType::Volumes => "Docker Volume Management",
            TabType::Networks => "Docker Network Management",
            TabType::System => "Docker System Dashboard",
        }
    }

    /// next tab, wraps around at the end
    pub fn next(&self) -> TabType {
        let tabs = Self::all();
        let current_index = tabs.iter().position(|&tab| tab == *self).unwrap_or(0);
        let next_index = (current_index + 1) % tabs.len();
        tabs[next_index]
    }

    /// previous tab, wraps to the last one
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
    /// connects to docker and builds every tab up front
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

        // grab version once here, we cache it and dont ask again
        let docker_version = match docker_client.inner().version().await {
            Ok(v) => v.version.unwrap_or_else(|| "unknown".to_string()),
            Err(_) => "unknown".to_string(),
        };

        let containers_tab =
            EnhancedContainersTab::new(docker_client.clone(), theme.clone()).await?;
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
            theme,
            mouse_enabled,
            nav_area: None,
            content_area: None,
            toasts: Vec::new(),
            spinner_frame: 0,
            is_loading: false,
            docker_connected: true,
            docker_version,
        })
    }

    /// sets up the terminal, runs the loop, then restores everything on exit
    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        if self.mouse_enabled {
            stdout().execute(EnableMouseCapture)?;
        }
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        self.add_toast(
            "Welcome to Docsee! Press ? for help".to_string(),
            ToastSeverity::Info,
        );

        let result = self.main_loop(&mut terminal).await;

        // whatever happened in the loop, undo the terminal setup so shell is usable again
        if self.mouse_enabled {
            stdout().execute(DisableMouseCapture)?;
        }
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        result
    }

    async fn main_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

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

    fn add_toast(&mut self, message: String, severity: ToastSeverity) {
        self.toasts.push(Toast::new(message, severity));
    }

    /// key router - shell mode grabs keys raw, otherwise globals then the active tab
    async fn handle_key_event(&mut self, key: crate::events::Key) -> Result<()> {
        use crate::events::Key;

        // cheatsheet is modal, only esc closes it and everything else is swallowed
        if self.show_cheatsheet {
            if key == Key::Esc {
                self.show_cheatsheet = false;
            }
            return Ok(());
        }

        self.in_container_subview =
            self.current_tab == TabType::Containers && self.containers_tab.is_in_subview();

        // shell needs raw keys (ctrl-c etc), so intercept before global handling steals them
        if self.in_container_subview
            && self.containers_tab.get_view_mode() == &ContainerViewMode::Shell
        {
            let shell_exit = self.containers_tab.handle_shell_key_raw(key).await?;
            if shell_exit {
                return Ok(());
            }
            return Ok(());
        }

        if !self.in_container_subview {
            match key {
                Key::Quit => {
                    self.should_quit = true;
                    return Ok(());
                }
                Key::Cheatsheet | Key::Char('?') => {
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
                Key::Char('?') => {
                    self.show_cheatsheet = true;
                    return Ok(());
                }
                _ => {}
            }
        }

        // nothing global matched, forward to whichever tab is active
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

    /// runs on every tick - refreshes the active tab and notices docker dropping
    async fn handle_tick(&mut self) -> Result<()> {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        self.toasts.retain(|t| !t.is_expired());

        let result = match self.current_tab {
            TabType::Containers => self.containers_tab.refresh().await,
            TabType::Images => self.images_tab.refresh().await,
            TabType::Volumes => self.volumes_tab.refresh().await,
            TabType::Networks => self.networks_tab.refresh().await,
            TabType::System => self.system_tab.refresh().await,
        };

        if let Err(e) = result {
            // no clean error type from the docker layer, so sniff the message string
            let msg = format!("{}", e);
            if msg.contains("connection")
                || msg.contains("refused")
                || msg.contains("daemon")
                || msg.contains("socket")
            {
                self.docker_connected = false;
                self.add_toast(
                    "Docker disconnected - reconnect and press any key".to_string(),
                    ToastSeverity::Error,
                );
            }
        } else {
            self.docker_connected = true;
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, mouse_event: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};

        let col = mouse_event.column;
        let row = mouse_event.row;

        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(nav) = self.nav_area {
                    if row >= nav.y
                        && row < nav.y + nav.height
                        && col >= nav.x
                        && col < nav.x + nav.width
                    {
                        // figure out which tab got clicked from the x offset
                        let tabs = TabType::all();
                        let tab_width = nav.width / tabs.len() as u16;
                        let clicked_tab = ((col - nav.x) / tab_width) as usize;
                        if clicked_tab < tabs.len() {
                            self.current_tab = tabs[clicked_tab];
                        }
                        return;
                    }
                }

                if let Some(content) = self.content_area {
                    if row >= content.y && row < content.y + content.height {
                        let clicked_row = (row - content.y) as usize;
                        // first 3 rows are header/border, so subtract to get the actual item
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
            MouseEventKind::ScrollUp => match self.current_tab {
                TabType::Containers => self.containers_tab.scroll_up(),
                TabType::Images => self.images_tab.scroll_up(),
                TabType::Volumes => self.volumes_tab.scroll_up(),
                TabType::Networks => self.networks_tab.scroll_up(),
                _ => {}
            },
            MouseEventKind::ScrollDown => match self.current_tab {
                TabType::Containers => self.containers_tab.scroll_down(),
                TabType::Images => self.images_tab.scroll_down(),
                TabType::Volumes => self.volumes_tab.scroll_down(),
                TabType::Networks => self.networks_tab.scroll_down(),
                _ => {}
            },
            _ => {}
        }
    }

    fn tab_count(&self, tab: TabType) -> String {
        match tab {
            TabType::Containers => {
                let (total, running) = self.containers_tab.container_counts();
                format!("{}/{}", running, total)
            }
            TabType::Images => format!("{}", self.images_tab.count()),
            TabType::Volumes => format!("{}", self.volumes_tab.count()),
            TabType::Networks => format!("{}", self.networks_tab.count()),
            TabType::System => String::new(),
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let size = frame.area();
        // on short terminals we drop the title bar to save a row
        let is_compact = size.height < 35;

        let title_height = if is_compact { 0 } else { 3 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(title_height),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(size);

        // stash these so handle_mouse_event can map clicks back to a region
        self.nav_area = Some(chunks[1]);
        self.content_area = Some(chunks[3]);

        if !is_compact {
            self.draw_compact_title(frame, chunks[0]);
        }

        self.draw_tab_bar(frame, chunks[1]);
        self.draw_status_bar(frame, chunks[2]);

        if self.show_cheatsheet {
            self.cheatsheet.draw(frame, chunks[3]);
        } else {
            match self.current_tab {
                TabType::Containers => {
                    self.containers_tab.draw(frame, chunks[3]);
                }
                TabType::Images => {
                    self.images_tab.draw(frame, chunks[3]);
                }
                TabType::Volumes => {
                    self.volumes_tab.draw(frame, chunks[3]);
                }
                TabType::Networks => {
                    self.networks_tab.draw(frame, chunks[3]);
                }
                TabType::System => {
                    self.system_tab.draw(frame, chunks[3]);
                }
            }
        }

        self.draw_footer(frame, chunks[4]);

        // toasts drawn last so they sit on top of everything else
        self.draw_toasts(frame, size);
    }

    /// single-line breadcrumb title at the very top
    fn draw_compact_title(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;

        let mut spans = vec![
            Span::styled(
                " DOCSEE ",
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " v2.0 ",
                Style::default().fg(t.muted).add_modifier(Modifier::ITALIC),
            ),
        ];

        // only show the container > view breadcrumb when we're inside a subview
        if self.in_container_subview {
            spans.push(Span::styled(" > ", Style::default().fg(t.muted)));
            spans.push(Span::styled("Containers", Style::default().fg(t.info)));
            spans.push(Span::styled(" > ", Style::default().fg(t.muted)));

            if let Some(container_name) = self.containers_tab.get_selected_container_name() {
                spans.push(Span::styled(container_name, Style::default().fg(t.fg)));
                spans.push(Span::styled(" > ", Style::default().fg(t.muted)));
            }

            spans.push(Span::styled(
                self.containers_tab.get_view_mode().name(),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ));
        }

        let title = Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(t.border))
                .border_type(BorderType::Rounded),
        );

        frame.render_widget(title, area);
    }

    /// tab bar with the live resource counts next to each name
    fn draw_tab_bar(&mut self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;
        let tabs = TabType::all();

        let mut spans: Vec<Span> = Vec::new();
        for (i, &tab) in tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" | ", Style::default().fg(t.border)));
            }

            let is_active = tab == self.current_tab;
            let count = self.tab_count(tab);

            let label = if count.is_empty() {
                format!(" {} ", tab.name())
            } else {
                format!(" {} ({}) ", tab.name(), count)
            };

            let style = if is_active {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.muted)
            };

            spans.push(Span::styled(label, style));
        }

        spans.push(Span::styled(
            "  [</>] switch",
            Style::default().fg(t.border),
        ));

        let tab_bar = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);

        frame.render_widget(tab_bar, area);
    }

    /// status bar - connection dot, container counts and docker version
    fn draw_status_bar(&self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;

        let connection_status = if self.docker_connected {
            Span::styled("● Connected", Style::default().fg(t.success))
        } else {
            Span::styled("● Disconnected", Style::default().fg(t.error))
        };

        let spinner = if self.is_loading {
            Span::styled(
                format!(" {} ", SPINNER_FRAMES[self.spinner_frame]),
                Style::default().fg(t.info),
            )
        } else {
            Span::raw("")
        };

        let (total, running) = self.containers_tab.container_counts();
        let container_stats = Span::styled(
            format!("  {} running / {} total", running, total),
            Style::default().fg(t.muted),
        );

        let version = Span::styled(
            format!("  Docker {}", self.docker_version),
            Style::default().fg(t.border),
        );

        let status_bar = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            connection_status,
            container_stats,
            version,
            spinner,
        ]));

        frame.render_widget(status_bar, area);
    }

    /// footer showing whichever shortcuts make sense for the current screen
    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let t = &self.theme;

        let footer_text = if self.in_container_subview {
            match self.containers_tab.get_view_mode() {
                ContainerViewMode::Logs => "Esc back | f follow | t timestamps | x export | ? more",
                ContainerViewMode::Shell => "Esc back | F1 mode | Tab shell | ? more",
                ContainerViewMode::Stats => "Esc back | </> view | r reset | p pause | ? more",
                ContainerViewMode::Inspect => "Esc back | </> section | j/k scroll | ? more",
                ContainerViewMode::Topology => "Esc back | j/k scroll | r refresh | ? more",
                _ => "Esc back | ? help",
            }
        } else if self.show_cheatsheet {
            "Esc close"
        } else {
            match self.current_tab {
                TabType::Containers => {
                    "q quit | ? help | / search | l logs | e shell | s stats | Enter inspect"
                }
                TabType::Images => "q quit | ? help | D delete | P pull | R run | p prune",
                TabType::System => "q quit | ? help | </> views | r refresh",
                _ => "q quit | ? help | D delete | p prune",
            }
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(t.muted))
            .alignment(Alignment::Center);

        frame.render_widget(footer, area);
    }

    /// draws the newest toast in the bottom-right, they auto expire on tick
    fn draw_toasts(&self, frame: &mut Frame, area: Rect) {
        if self.toasts.is_empty() {
            return;
        }

        // only render the latest one, stacking them just clutters the corner
        if let Some(toast) = self.toasts.last() {
            let toast_width = (toast.message.len() as u16 + 4).min(area.width.saturating_sub(4));
            let toast_height = 1;
            let x = area.width.saturating_sub(toast_width + 2);
            let y = area.height.saturating_sub(toast_height + 2);

            let toast_area = Rect::new(x, y, toast_width, toast_height);
            let color = toast.color(&self.theme);

            let toast_widget = Paragraph::new(format!(" {} ", toast.message))
                .style(Style::default().fg(color).add_modifier(Modifier::BOLD));

            frame.render_widget(toast_widget, toast_area);
        }
    }
}
