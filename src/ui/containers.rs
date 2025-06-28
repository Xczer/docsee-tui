use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::{
    docker::{containers::Container, DockerClient},
    events::Key,
};

// Import our new Phase 2 components
use crate::ui::logs_viewer::LogsViewer;
use crate::ui::shell_executor::ShellExecutor;
use crate::ui::stats_viewer::StatsViewer;
use crate::ui::search_filter::AdvancedSearch;

/// Enhanced containers tab with Phase 2 features
pub struct EnhancedContainersTab {
    /// Docker client for operations
    docker_client: DockerClient,
    /// List of all containers
    all_containers: Vec<Container>,
    /// Filtered containers (based on search/filter)
    filtered_containers: Vec<Container>,
    /// Table state for selection
    table_state: TableState,
    /// Status message to show
    status_message: Option<String>,
    /// Current view mode
    view_mode: ContainerViewMode,
    /// Logs viewer
    logs_viewer: LogsViewer,
    /// Shell executor
    shell_executor: ShellExecutor,
    /// Stats viewer
    stats_viewer: StatsViewer,
    /// Advanced search and filter
    search_filter: AdvancedSearch,
}

/// Different view modes for the containers tab
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerViewMode {
    List,
    Logs,
    Shell,
    Stats,
}

impl ContainerViewMode {
    pub fn name(&self) -> &'static str {
        match self {
            ContainerViewMode::List => "Container List",
            ContainerViewMode::Logs => "Container Logs",
            ContainerViewMode::Shell => "Shell Access",
            ContainerViewMode::Stats => "Resource Stats",
        }
    }
}

impl EnhancedContainersTab {
    /// Create a new enhanced containers tab
    pub async fn new(docker_client: DockerClient) -> Result<Self> {
        let logs_viewer = LogsViewer::new(docker_client.clone());
        let shell_executor = ShellExecutor::new(docker_client.clone());
        let stats_viewer = StatsViewer::new(docker_client.clone());
        let search_filter = AdvancedSearch::new();

        let mut tab = Self {
            docker_client,
            all_containers: Vec::new(),
            filtered_containers: Vec::new(),
            table_state: TableState::default(),
            status_message: None,
            view_mode: ContainerViewMode::List,
            logs_viewer,
            shell_executor,
            stats_viewer,
            search_filter,
        };

        // Load initial data
        tab.refresh().await?;

        // Select first container if any exist
        if !tab.filtered_containers.is_empty() {
            tab.table_state.select(Some(0));
        }

        Ok(tab)
    }

    /// Refresh container data from Docker
    pub async fn refresh(&mut self) -> Result<()> {
        match self.docker_client.list_containers().await {
            Ok(containers) => {
                // Remember current selection
                let selected_id = self.get_selected_container().map(|c| c.id.clone());

                self.all_containers = containers;

                // Update search suggestions
                self.search_filter.update_suggestions(&self.all_containers);

                // Apply current filters
                self.apply_filters();

                self.status_message = None;

                // Restore selection or select first item
                if let Some(id) = selected_id {
                    // Try to find the same container
                    let new_index = self.filtered_containers.iter().position(|c| c.id == id);
                    self.table_state.select(new_index.or(Some(0)));
                } else if !self.filtered_containers.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading containers: {}", e));
            }
        }

        // Update components that need regular refresh
        match self.view_mode {
            ContainerViewMode::Logs => {
                self.logs_viewer.update().await?;
            }
            ContainerViewMode::Stats => {
                self.stats_viewer.update().await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Apply current search and filter settings
    fn apply_filters(&mut self) {
        self.filtered_containers = self.search_filter.filter_containers(&self.all_containers);
    }

    /// Handle key press events
    pub async fn handle_key(&mut self, key: Key) -> Result<()> {
        // Handle view-specific keys first
        match self.view_mode {
            ContainerViewMode::List => {
                // Handle search/filter keys
                if self.search_filter.handle_key(key) {
                    if !self.search_filter.is_search_active() {
                        // Search completed, apply filters
                        self.apply_filters();
                        // Reset table selection
                        if !self.filtered_containers.is_empty() {
                            self.table_state.select(Some(0));
                        }
                    }
                    return Ok(());
                }

                // Handle list navigation and actions
                match key {
                    Key::Up => self.move_selection_up(),
                    Key::Down => self.move_selection_down(),
                    Key::Start => self.start_selected_container().await?,
                    Key::Stop => self.stop_selected_container().await?,
                    Key::Restart => self.restart_selected_container().await?,
                    Key::DeleteItem => self.delete_selected_container().await?,
                    Key::Logs => self.enter_logs_view().await?,
                    Key::Exec => self.enter_shell_view().await?,
                    Key::Char('s') => self.enter_stats_view().await?,
                    Key::Char('i') => self.start_interactive_shell().await?,
                    _ => {}
                }
            }
            ContainerViewMode::Logs => {
                match key {
                    Key::Esc => self.exit_to_list_view().await?,
                    _ => {
                        self.logs_viewer.handle_key(key).await?;
                    }
                }
            }
            ContainerViewMode::Shell => {
                match self.shell_executor.handle_key(key).await? {
                    true => self.exit_to_list_view().await?, // Exit requested
                    false => {} // Continue in shell mode
                }
            }
            ContainerViewMode::Stats => {
                match key {
                    Key::Esc => self.exit_to_list_view().await?,
                    _ => {
                        self.stats_viewer.handle_key(key).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle raw key events for shell mode (bypasses key conversion)
    pub async fn handle_shell_key_raw(&mut self, key: Key) -> Result<bool> {
        match self.view_mode {
            ContainerViewMode::Shell => {
                // Convert action keys back to characters for shell input
                let shell_key = match key {
                    Key::Cheatsheet => Key::Char('c'),
                    Key::Logs => Key::Char('l'),
                    Key::Stop => Key::Char('d'),
                    Key::Restart => Key::Char('r'),
                    Key::Start => Key::Char('u'),
                    Key::Exec => Key::Char('e'),
                    Key::Prune => Key::Char('p'),
                    // Keep other keys as they are
                    _ => key,
                };
                
                match self.shell_executor.handle_key(shell_key).await? {
                    true => {
                        self.exit_to_list_view().await?;
                        Ok(true) // Exit requested
                    }
                    false => Ok(false), // Continue in shell mode
                }
            }
            _ => Ok(false), // Not in shell mode
        }
    }

    /// Draw the enhanced containers tab
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match self.view_mode {
            ContainerViewMode::List => self.draw_list_view(frame, area),
            ContainerViewMode::Logs => self.logs_viewer.draw(frame, area),
            ContainerViewMode::Shell => self.shell_executor.draw(frame, area),
            ContainerViewMode::Stats => self.stats_viewer.draw(frame, area),
        }
    }

    /// Draw the container list view with search/filter
    fn draw_list_view(&mut self, frame: &mut Frame, area: Rect) {
        // Split area for search controls and table
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Search and filter controls
                Constraint::Min(0),    // Container table
            ])
            .split(area);

        // Draw search and filter controls
        self.search_filter.draw(frame, chunks[0]);

        // Draw container table
        self.draw_container_table(frame, chunks[1]);
    }

    /// Draw the container table
    fn draw_container_table(&mut self, frame: &mut Frame, area: Rect) {
        // Create table rows
        let rows: Vec<Row> = self
            .filtered_containers
            .iter()
            .map(|container| {
                let style = match container.state {
                    crate::docker::containers::ContainerState::Running => {
                        Style::default().fg(Color::Green)
                    }
                    crate::docker::containers::ContainerState::Stopped => {
                        Style::default().fg(Color::Red)
                    }
                    crate::docker::containers::ContainerState::Paused => {
                        Style::default().fg(Color::Yellow)
                    }
                    _ => Style::default().fg(Color::Gray),
                };

                Row::new(vec![
                    Cell::from(container.id.clone()),
                    Cell::from(container.name.clone()),
                    Cell::from(container.image.clone()),
                    Cell::from(container.state.display()),
                    Cell::from(container.ports.clone()),
                    Cell::from(container.created.clone()),
                ])
                .style(style)
            })
            .collect();

        // Create table headers
        let header = Row::new(vec![
            Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Image").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Ports").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Created").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        // Build the title string
        let total_count = self.all_containers.len();
        let filtered_count = self.filtered_containers.len();
        let running_count = self
            .filtered_containers
            .iter()
            .filter(|c| c.state == crate::docker::containers::ContainerState::Running)
            .count();

        let filter_desc = self.search_filter.get_filter_description();
        let title_text = if total_count == filtered_count {
            format!("Containers ({} total, {} running)", total_count, running_count)
        } else {
            format!(
                "Containers ({}/{} shown, {} running) - Filter: {}",
                filtered_count, total_count, running_count, filter_desc
            )
        };

        let title = if let Some(ref message) = self.status_message {
            format!("{} - {}", title_text, message)
        } else {
            title_text
        };

        // Create the table widget
        let table = Table::new(
            rows,
            [
                Constraint::Length(12), // ID
                Constraint::Length(20), // Name
                Constraint::Length(25), // Image
                Constraint::Length(15), // Status
                Constraint::Length(15), // Ports
                Constraint::Length(20), // Created
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(">> ");

        // Render the table
        frame.render_stateful_widget(table, area, &mut self.table_state);

        // Draw additional help info if not searching
        if !self.search_filter.is_search_active() {
            self.draw_help_footer(frame, area);
        }
    }

    /// Draw help information at the bottom
    fn draw_help_footer(&self, _frame: &mut Frame, _area: Rect) {
        // This would overlay help text at the bottom of the table area
        // For brevity, we'll skip the implementation here
        // but it would show key shortcuts like:
        // "l: Logs | e: Shell | s: Stats | i: Interactive | /: Search | f: Filter"
    }

    /// Enter logs view for selected container
    async fn enter_logs_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.logs_viewer.start_logs(container).await?;
            self.view_mode = ContainerViewMode::Logs;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Enter shell view for selected container
    async fn enter_shell_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.shell_executor.set_container(container);
            self.view_mode = ContainerViewMode::Shell;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Enter stats view for selected container
    async fn enter_stats_view(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container().cloned() {
            self.stats_viewer.start_monitoring(container).await?;
            self.view_mode = ContainerViewMode::Stats;
            self.status_message = None;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Start interactive shell (drops out of TUI temporarily)
    async fn start_interactive_shell(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            self.shell_executor.start_interactive_shell(container).await?;
        } else {
            self.status_message = Some("No container selected".to_string());
        }
        Ok(())
    }

    /// Exit back to list view
    async fn exit_to_list_view(&mut self) -> Result<()> {
        // Stop any active monitoring/streaming
        match self.view_mode {
            ContainerViewMode::Logs => {
                self.logs_viewer.stop_logs().await;
            }
            ContainerViewMode::Stats => {
                self.stats_viewer.stop_monitoring().await;
            }
            _ => {}
        }

        self.view_mode = ContainerViewMode::List;
        self.status_message = None;
        Ok(())
    }

    /// Get the currently selected container
    fn get_selected_container(&self) -> Option<&Container> {
        self.table_state
            .selected()
            .and_then(|index| self.filtered_containers.get(index))
    }

    /// Move selection up
    fn move_selection_up(&mut self) {
        if self.filtered_containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = if selected == 0 {
            self.filtered_containers.len() - 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(new_index));
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.filtered_containers.is_empty() {
            return;
        }

        let selected = self.table_state.selected().unwrap_or(0);
        let new_index = (selected + 1) % self.filtered_containers.len();
        self.table_state.select(Some(new_index));
    }

    /// Start the selected container
    async fn start_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            match self.docker_client.start_container(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Started container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to start '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Stop the selected container
    async fn stop_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            match self.docker_client.stop_container(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Stopped container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to stop '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Restart the selected container
    async fn restart_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            match self.docker_client.restart_container(&id).await {
                Ok(_) => {
                    self.status_message = Some(format!("Restarted container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to restart '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Delete the selected container
    async fn delete_selected_container(&mut self) -> Result<()> {
        if let Some(container) = self.get_selected_container() {
            let id = container.id.clone();
            let name = container.name.clone();

            // For safety, only allow deletion of stopped containers
            if container.state == crate::docker::containers::ContainerState::Running {
                self.status_message = Some(format!("Cannot delete running container '{}'. Stop it first.", name));
                return Ok(());
            }

            match self.docker_client.remove_container(&id, false).await {
                Ok(_) => {
                    self.status_message = Some(format!("Deleted container '{}'", name));
                    self.refresh().await?;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to delete '{}': {}", name, e));
                }
            }
        }
        Ok(())
    }

    /// Get current view mode
    pub fn get_view_mode(&self) -> &ContainerViewMode {
        &self.view_mode
    }

    /// Get status for display in parent UI
    pub fn get_status(&self) -> Option<String> {
        match &self.view_mode {
            ContainerViewMode::List => self.status_message.clone(),
            ContainerViewMode::Logs => {
                if let Some(container) = self.logs_viewer.get_container() {
                    Some(format!("Viewing logs for '{}'", container.name))
                } else {
                    Some("Logs viewer".to_string())
                }
            }
            ContainerViewMode::Shell => {
                if let Some(container) = self.shell_executor.get_container() {
                    Some(format!("Shell access to '{}'", container.name))
                } else {
                    Some("Shell executor".to_string())
                }
            }
            ContainerViewMode::Stats => {
                if let Some(container) = self.stats_viewer.get_container() {
                    Some(format!("Monitoring stats for '{}'", container.name))
                } else {
                    Some("Resource stats".to_string())
                }
            }
        }
    }

    /// Check if we're in a sub-view (not the main list)
    pub fn is_in_subview(&self) -> bool {
        self.view_mode != ContainerViewMode::List
    }

    /// Force exit from any sub-view (useful for global navigation)
    pub async fn force_exit_subview(&mut self) -> Result<()> {
        if self.is_in_subview() {
            self.exit_to_list_view().await?;
        }
        Ok(())
    }
}
